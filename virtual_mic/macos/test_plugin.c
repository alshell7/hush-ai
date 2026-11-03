// Loads HushMic.driver without Core Audio and checks its device properties and audio path.
//   clang -framework CoreAudio -framework CoreFoundation -o target/test_plugin virtual_mic/macos/test_plugin.c
//   target/test_plugin target/HushMic.driver/Contents/MacOS/HushMic
#include <CoreAudio/AudioServerPlugIn.h>
#include <dlfcn.h>
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

    // One input device, no output streams, 48 kHz mono float32.
    CHECK(Count(kAudioObjectPlugInObject, kAudioPlugInPropertyDeviceList, kAudioObjectPropertyScopeGlobal) == 1);
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
    puts("HushMic plug-in: all checks passed");
    return 0;
}
