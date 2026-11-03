// Loads HushMic.driver without Core Audio and checks the properties and audio paths of its two
// devices: Hush AI Microphone (2, stream 3) and Hush AI Speaker (4, stream 5).
//   clang -framework CoreAudio -framework CoreFoundation -o target/test_plugin virtual_mic/macos/test_plugin.c
//   target/test_plugin target/HushMic.driver/Contents/MacOS/HushMic
#include <CoreAudio/AudioServerPlugIn.h>
#include <dlfcn.h>
#include <mach/mach_time.h>
#include <math.h>
#include <fcntl.h>
#include <stdatomic.h>
#include <stdio.h>
#include <stdlib.h>
#include <sys/mman.h>
#include <unistd.h>
#include "../shared.h"

#define CHECK(cond) do { if (!(cond)) { fprintf(stderr, "FAIL %s:%d: %s\n", __FILE__, __LINE__, #cond); exit(1); } } while (0)

static AudioServerPlugInDriverRef driver;
static AudioServerPlugInIOCycleInfo cycle;
static AudioServerPlugInHostInterface host;

static UInt32 GetU32(AudioObjectID obj, AudioObjectPropertySelector sel, AudioObjectPropertyScope scope) {
    AudioObjectPropertyAddress a = {sel, scope, kAudioObjectPropertyElementMain};
    CHECK((*driver)->HasProperty(driver, obj, 0, &a));
    UInt32 value = 0, size = 0;
    CHECK((*driver)->GetPropertyData(driver, obj, 0, &a, 0, NULL, sizeof(value), &size, &value) == 0);
    return value;
}

static UInt32 Count(AudioObjectID obj, AudioObjectPropertySelector sel, AudioObjectPropertyScope scope) {
    AudioObjectPropertyAddress a = {sel, scope, kAudioObjectPropertyElementMain};
    UInt32 size = 0;
    CHECK((*driver)->GetPropertyDataSize(driver, obj, 0, &a, 0, NULL, &size) == 0);
    return size / sizeof(AudioObjectID);
}

static const char *Name(AudioObjectID obj) {
    static char text[64];
    AudioObjectPropertyAddress a = {kAudioObjectPropertyName, kAudioObjectPropertyScopeGlobal, kAudioObjectPropertyElementMain};
    CFStringRef value = NULL;
    UInt32 size = 0;
    CHECK((*driver)->GetPropertyData(driver, obj, 0, &a, 0, NULL, sizeof(value), &size, &value) == 0);
    CHECK(CFStringGetCString(value, text, sizeof(text), kCFStringEncodingUTF8));
    CFRelease(value);
    return text;
}

static AudioObjectID FromUID(const char *uid) {
    AudioObjectPropertyAddress a = {kAudioPlugInPropertyTranslateUIDToDevice, kAudioObjectPropertyScopeGlobal, kAudioObjectPropertyElementMain};
    CFStringRef q = CFStringCreateWithCString(NULL, uid, kCFStringEncodingUTF8);
    AudioObjectID dev = 0;
    UInt32 size = 0;
    CHECK((*driver)->GetPropertyData(driver, kAudioObjectPlugInObject, 0, &a, sizeof(q), &q, sizeof(dev), &size, &dev) == 0);
    CFRelease(q);
    return dev;
}

static Boolean WillDo(AudioObjectID dev, UInt32 op) {
    Boolean willDo = false, inPlace = false;
    CHECK((*driver)->WillDoIOOperation(driver, dev, 0, op, &willDo, &inPlace) == 0);
    return willDo;
}

static void Read(float *out, UInt32 frames, Float64 time) {
    cycle.mInputTime.mSampleTime = time;
    CHECK((*driver)->DoIOOperation(driver, 2, 3, 0, kAudioServerPlugInIOOperationReadInput, frames, &cycle, out, NULL) == 0);
}

/* Marker values stay inside [-1, 1], which the plug-in enforces. */
static float V(float x) { return x * 1e-5f; }

static void Write(HushMicShared *shared, uint64_t count, float first) {
    uint64_t w = atomic_load((_Atomic uint64_t *)&shared->write_pos);
    for (uint64_t i = 0; i < count; i++) shared->samples[(w + i) & (HUSH_MIC_CAPACITY - 1)] = V(first + (float)i);
    atomic_store_explicit((_Atomic uint64_t *)&shared->write_pos, w + count, memory_order_release);
}

int main(int argc, char **argv) {
    CHECK(argc == 2);
    void *lib = dlopen(argv[1], RTLD_NOW | RTLD_LOCAL);
    CHECK(lib);
    void *(*create)(CFAllocatorRef, CFUUIDRef) = dlsym(lib, "HushMic_Create");
    CHECK(create);
    CHECK(create(NULL, kAudioServerPlugInDriverInterfaceUUID) == NULL);
    driver = create(NULL, kAudioServerPlugInTypeUUID);
    CHECK(driver);
    CHECK((*driver)->Initialize(driver, &host) == 0);

    // Two devices: the microphone and the speaker.
    CHECK(Count(kAudioObjectPlugInObject, kAudioPlugInPropertyDeviceList, kAudioObjectPropertyScopeGlobal) == 2);
    CHECK(FromUID("HushAIMicrophone_UID") == 2 && FromUID("HushAISpeaker_UID") == 4 && FromUID("x") == 0);

    // The microphone: one input stream, no output streams, 48 kHz mono float32.
    AudioObjectPropertyAddress name = {kAudioObjectPropertyName, kAudioObjectPropertyScopeGlobal, kAudioObjectPropertyElementMain};
    CFStringRef value = NULL;
    UInt32 size = 0;
    CHECK((*driver)->GetPropertyData(driver, 2, 0, &name, 0, NULL, sizeof(value), &size, &value) == 0);
    char text[64];
    CHECK(CFStringGetCString(value, text, sizeof(text), kCFStringEncodingUTF8));
    CFRelease(value);
    CHECK(strcmp(text, "Hush AI Microphone") == 0);
    CHECK(Count(2, kAudioDevicePropertyStreams, kAudioObjectPropertyScopeInput) == 1);
    CHECK(Count(2, kAudioDevicePropertyStreams, kAudioObjectPropertyScopeOutput) == 0);
    CHECK(GetU32(2, kAudioDevicePropertyDeviceCanBeDefaultDevice, kAudioObjectPropertyScopeInput) == 1);
    CHECK(GetU32(3, kAudioStreamPropertyDirection, kAudioObjectPropertyScopeGlobal) == 1);
    AudioObjectPropertyAddress fmt = {kAudioStreamPropertyVirtualFormat, kAudioObjectPropertyScopeGlobal, kAudioObjectPropertyElementMain};
    AudioStreamBasicDescription format;
    CHECK((*driver)->GetPropertyData(driver, 3, 0, &fmt, 0, NULL, sizeof(format), &size, &format) == 0);
    CHECK(format.mSampleRate == 48000 && format.mChannelsPerFrame == 1 && format.mBitsPerChannel == 32);
    CHECK(WillDo(2, kAudioServerPlugInIOOperationReadInput) && !WillDo(2, kAudioServerPlugInIOOperationWriteMix));

    // The app side: open the region the plug-in created and write a ramp.
    int fd = shm_open(HUSH_MIC_SHM_NAME, O_RDWR);
    CHECK(fd >= 0);
    HushMicShared *shared = mmap(NULL, sizeof(HushMicShared), PROT_READ | PROT_WRITE, MAP_SHARED, fd, 0);
    CHECK(shared != MAP_FAILED);
    CHECK(shared->magic == HUSH_MIC_MAGIC && shared->capacity == HUSH_MIC_CAPACITY);
    memset(shared->samples, 0, sizeof(shared->samples)); // The app clears old audio on open.
    CHECK((*driver)->StartIO(driver, 2, 0) == 0);
    float out[512], again[512];
    Read(out, 512, 0); // Nothing written yet: silence.
    for (int i = 0; i < 512; i++) CHECK(out[i] == 0);

    // Audio written before the plug-in started reading is old: silence until the app writes again.
    CHECK((*driver)->StopIO(driver, 2, 0) == 0);
    CHECK((*driver)->StartIO(driver, 2, 0) == 0);
    uint64_t w = atomic_load((_Atomic uint64_t *)&shared->write_pos); // value(w + p) == p below
    Write(shared, 1000, 0);
    Read(out, 512, 1000);
    for (int i = 0; i < 512; i++) CHECK(out[i] == 0);

    // The app writes again: audio continues 10 ms behind the newest sample.
    Write(shared, 512, 1000);
    Read(out, 512, 1512);
    for (int i = 0; i < 512; i++) CHECK(out[i] == V((float)(520 + i)));
    CHECK(atomic_load((_Atomic uint64_t *)&shared->read_pos) == w + 1032);
    CHECK(shared->read_frames == 512);

    // A second client reading the same device time gets the same audio and moves nothing.
    Read(again, 512, 1512);
    CHECK(memcmp(out, again, sizeof(out)) == 0);
    CHECK(atomic_load((_Atomic uint64_t *)&shared->read_pos) == w + 1032);

    // The next period: 480 written samples remain, then silence (the app is late).
    Read(out, 512, 2024);
    for (int i = 0; i < 480; i++) CHECK(out[i] == V((float)(1032 + i)));
    for (int i = 480; i < 512; i++) CHECK(out[i] == 0);

    // The app wrote far ahead: skip to recent audio.
    Write(shared, 10000, 1512);
    Read(out, 512, 2536);
    CHECK(out[0] == V(10520.0f) && out[511] == V(11031.0f));

    // The zero time stamp moves forward with the host clock.
    Float64 t1, t2;
    UInt64 h1, h2, seed;
    CHECK((*driver)->GetZeroTimeStamp(driver, 2, 0, &t1, &h1, &seed) == 0);
    usleep(400000);
    CHECK((*driver)->GetZeroTimeStamp(driver, 2, 0, &t2, &h2, &seed) == 0);
    CHECK(t2 > t1 && h2 > h1);
    CHECK((*driver)->StopIO(driver, 2, 0) == 0);

    // The speaker: one output stream, 48 kHz stereo float32, never the system (alert) device.
    CHECK(strcmp(Name(4), "Hush AI Speaker") == 0);
    CHECK(Count(4, kAudioDevicePropertyStreams, kAudioObjectPropertyScopeOutput) == 1);
    CHECK(Count(4, kAudioDevicePropertyStreams, kAudioObjectPropertyScopeInput) == 0);
    CHECK(GetU32(4, kAudioDevicePropertyDeviceCanBeDefaultDevice, kAudioObjectPropertyScopeOutput) == 1);
    CHECK(GetU32(4, kAudioDevicePropertyDeviceCanBeDefaultSystemDevice, kAudioObjectPropertyScopeOutput) == 0);
    CHECK(GetU32(5, kAudioStreamPropertyDirection, kAudioObjectPropertyScopeGlobal) == 0);
    CHECK(GetU32(5, kAudioObjectPropertyOwner, kAudioObjectPropertyScopeGlobal) == 4);
    CHECK((*driver)->GetPropertyData(driver, 5, 0, &fmt, 0, NULL, sizeof(format), &size, &format) == 0);
    CHECK(format.mSampleRate == 48000 && format.mChannelsPerFrame == 2 && format.mBytesPerFrame == 8);
    CHECK(WillDo(4, kAudioServerPlugInIOOperationWriteMix) && !WillDo(4, kAudioServerPlugInIOOperationReadInput));

    // The app side of the speaker: the plug-in writes the mono mix, the app reads.
    fd = shm_open(HUSH_SPEAKER_SHM_NAME, O_RDWR);
    CHECK(fd >= 0);
    HushSpeakerShared *speaker = mmap(NULL, sizeof(HushSpeakerShared), PROT_READ | PROT_WRITE, MAP_SHARED, fd, 0);
    CHECK(speaker != MAP_FAILED);
    CHECK(speaker->magic == HUSH_SPEAKER_MAGIC && speaker->capacity == HUSH_SPEAKER_CAPACITY &&
          speaker->sample_rate == 48000);
    CHECK((*driver)->StartIO(driver, 4, 0) == 0);
    CHECK(GetU32(4, kAudioDevicePropertyDeviceIsRunning, kAudioObjectPropertyScopeGlobal) == 1);
    CHECK(GetU32(2, kAudioDevicePropertyDeviceIsRunning, kAudioObjectPropertyScopeGlobal) == 0);
    CHECK((*driver)->GetZeroTimeStamp(driver, 4, 0, &t1, &h1, &seed) == 0);
    static float stereo[2 * (HUSH_SPEAKER_CAPACITY + 100)];
    for (int i = 0; i < 512; i++) { stereo[2 * i] = V((float)i); stereo[2 * i + 1] = V(3.0f * (float)i); }
    stereo[0] = NAN; stereo[1] = 0.0f;    // not finite: 0
    stereo[2] = 4.0f; stereo[3] = 4.0f;   // too loud: 1
    stereo[4] = INFINITY; stereo[5] = -INFINITY; // mix not a number: 0
    uint64_t sw = atomic_load((_Atomic uint64_t *)&speaker->write_pos);
    uint64_t before = mach_absolute_time();
    CHECK((*driver)->DoIOOperation(driver, 4, 5, 0, kAudioServerPlugInIOOperationWriteMix, 512, &cycle, stereo, NULL) == 0);
    CHECK(atomic_load((_Atomic uint64_t *)&speaker->write_pos) == sw + 512);
    CHECK(speaker->write_frames == 512);
    CHECK(speaker->write_host_time >= before && speaker->write_host_time <= mach_absolute_time());
    const float *ring = speaker->samples;
    uint32_t mask = HUSH_SPEAKER_CAPACITY - 1;
    CHECK(ring[sw & mask] == 0.0f && ring[(sw + 1) & mask] == 1.0f && ring[(sw + 2) & mask] == 0.0f);
    for (int i = 3; i < 512; i++) CHECK(ring[(sw + i) & mask] == 0.5f * (V((float)i) + V(3.0f * (float)i)));
    // Other operations on the speaker do nothing.
    CHECK((*driver)->DoIOOperation(driver, 4, 5, 0, kAudioServerPlugInIOOperationReadInput, 512, &cycle, stereo, NULL) == 0);
    CHECK(atomic_load((_Atomic uint64_t *)&speaker->write_pos) == sw + 512);
    // More than the ring holds: only the newest capacity frames are kept.
    uint32_t big = HUSH_SPEAKER_CAPACITY + 100;
    for (uint32_t i = 0; i < big; i++) stereo[2 * i] = stereo[2 * i + 1] = V((float)i);
    CHECK((*driver)->DoIOOperation(driver, 4, 5, 0, kAudioServerPlugInIOOperationWriteMix, big, &cycle, stereo, NULL) == 0);
    uint64_t sw2 = atomic_load((_Atomic uint64_t *)&speaker->write_pos);
    CHECK(sw2 == sw + 512 + HUSH_SPEAKER_CAPACITY);
    CHECK(ring[(sw2 - 1) & mask] == V((float)(big - 1)) && ring[sw2 & mask] == V(100.0f));
    // Wrong device and stream pairs are refused.
    CHECK((*driver)->DoIOOperation(driver, 4, 3, 0, kAudioServerPlugInIOOperationWriteMix, 1, &cycle, stereo, NULL) != 0);
    CHECK((*driver)->DoIOOperation(driver, 2, 5, 0, kAudioServerPlugInIOOperationReadInput, 1, &cycle, stereo, NULL) != 0);
    CHECK((*driver)->StopIO(driver, 4, 0) == 0);
    CHECK(GetU32(4, kAudioDevicePropertyDeviceIsRunning, kAudioObjectPropertyScopeGlobal) == 0);
    puts("HushMic plug-in: all checks passed");
    return 0;
}
