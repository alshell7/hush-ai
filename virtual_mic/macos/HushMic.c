// Hush AI Microphone and Hush AI Speaker: a minimal Core Audio server plug-in with two
// virtual devices.
//
// Structure follows Apple's NullAudio sample ("Creating an Audio Server Driver Plug-in"):
// one plug-in object and two devices, no controls:
// - Hush AI Microphone: one input stream, 48 kHz mono float32. Audio comes from the Hush app
//   through the shared-memory ring HushMicShared in ../shared.h.
// - Hush AI Speaker: one output stream, 48 kHz stereo float32. Meeting apps play into it; the
//   plug-in mixes each IO cycle to mono and appends it to the ring HushSpeakerShared.
// The IO path only copies samples: no inference, no locks, no allocation, no system calls.

#include <CoreAudio/AudioServerPlugIn.h>
#include <mach/mach_time.h>
#include <pthread.h>
#include <stdatomic.h>
#include <string.h>
#include <sys/mman.h>
#include <errno.h>
#include <fcntl.h>
#include <sys/stat.h>
#include <unistd.h>
#include <os/log.h>
#include "../shared.h"
#include "../ring.h"

enum {
    kObjectID_PlugIn = kAudioObjectPlugInObject,
    kObjectID_Device = 2, /* Hush AI Microphone */
    kObjectID_Stream = 3,
    kObjectID_SpeakerDevice = 4, /* Hush AI Speaker */
    kObjectID_SpeakerStream = 5,
};

#define kDevice_Name "Hush AI Microphone"
#define kDevice_UID "HushAIMicrophone_UID"
#define kDevice_ModelUID "HushAIMicrophone_ModelUID"
#define kSpeaker_Name "Hush AI Speaker"
#define kSpeaker_UID "HushAISpeaker_UID"
#define kSpeaker_ModelUID "HushAISpeaker_ModelUID"
#define kManufacturer "Hush"
#define kZeroTimeStampPeriod 16384u


static pthread_mutex_t gStateMutex = PTHREAD_MUTEX_INITIALIZER;
static AudioServerPlugInHostRef gHost = NULL;
static UInt32 gRefCount = 0;
static Float64 gHostTicksPerFrame = 0;
/* One free-running clock per device. ioCount is guarded by gStateMutex. */
typedef struct {
    UInt32 ioCount;
    UInt64 anchorHostTime;
    UInt64 timeStampCount;
} DeviceClock;
static DeviceClock gMicClock, gSpeakerClock;
static HushMicShared *gShared = NULL;
static HushSpeakerShared *gSpeakerShared = NULL;
/* Speaker diagnostics, written on its IO thread and logged at stop. */
static UInt64 gSpeakerWrites, gSpeakerFrames;
/* Reader state (../ring.h). Used on the IO thread; reset when IO starts. */
static HushRingReader gReader;
/* Diagnostics, written on the IO thread and logged at stop. */
static UInt64 gTimeJumps, gReads;
static Float64 gLastTime = -1;
static os_log_t gLog;

// ---------------------------------------------------------------------------------------------
// Shared memory

// Create (or open) and map a shared-memory region. NULL on failure.
static void *MapShared(const char *name, size_t size) {
    int fd = shm_open(name, O_RDWR | O_CREAT, 0666);
    if (fd < 0) {
        os_log_error(gLog, "shm_open %{public}s failed: %{errno}d", name, errno);
        return NULL;
    }
    fchmod(fd, 0666); // The app runs as the user, the driver as _coreaudiod.
    // macOS rounds the size up to whole pages and allows setting it only once.
    struct stat st;
    if (fstat(fd, &st) != 0 || (st.st_size < (off_t)size && ftruncate(fd, size) != 0)) {
        os_log_error(gLog, "shared memory %{public}s size failed: %{errno}d", name, errno);
        close(fd);
        return NULL;
    }
    void *map = mmap(NULL, size, PROT_READ | PROT_WRITE, MAP_SHARED, fd, 0);
    close(fd);
    if (map == MAP_FAILED) {
        os_log_error(gLog, "mmap %{public}s failed: %{errno}d", name, errno);
        return NULL;
    }
    return map;
}

static void OpenShared(void) {
    if (gShared) return;
    HushMicShared *shared = MapShared(HUSH_MIC_SHM_NAME, sizeof(HushMicShared));
    if (!shared) return;
    if (shared->magic != HUSH_MIC_MAGIC || shared->version != HUSH_MIC_VERSION ||
        shared->capacity != HUSH_MIC_CAPACITY || shared->sample_rate != HUSH_MIC_RATE) {
        memset(shared, 0, sizeof(HushMicShared));
        shared->sample_rate = HUSH_MIC_RATE;
        shared->capacity = HUSH_MIC_CAPACITY;
        shared->version = HUSH_MIC_VERSION;
        atomic_thread_fence(memory_order_release);
        shared->magic = HUSH_MIC_MAGIC;
    }
    gShared = shared;
    os_log(gLog, "shared memory ready");
}

static void OpenSpeakerShared(void) {
    if (gSpeakerShared) return;
    HushSpeakerShared *shared = MapShared(HUSH_SPEAKER_SHM_NAME, sizeof(HushSpeakerShared));
    if (!shared) return;
    // A valid region keeps its positions across Core Audio restarts: an open reader stays valid.
    if (shared->magic != HUSH_SPEAKER_MAGIC || shared->version != HUSH_SPEAKER_VERSION ||
        shared->capacity != HUSH_SPEAKER_CAPACITY || shared->sample_rate != HUSH_SPEAKER_RATE) {
        memset(shared, 0, sizeof(HushSpeakerShared));
        shared->sample_rate = HUSH_SPEAKER_RATE;
        shared->capacity = HUSH_SPEAKER_CAPACITY;
        shared->version = HUSH_SPEAKER_VERSION;
        atomic_thread_fence(memory_order_release);
        shared->magic = HUSH_SPEAKER_MAGIC;
    }
    gSpeakerShared = shared;
    os_log(gLog, "speaker shared memory ready");
}

// Append the mix of one IO cycle (interleaved stereo) to the speaker ring as mono. The speaker's
// IO thread is the only writer. It never waits for the reader: it overwrites the oldest audio.
// Samples are made finite and clipped to [-1, 1]. Real-time safe.
static void WriteSpeaker(const float *in, UInt32 frames) {
    HushSpeakerShared *shared = gSpeakerShared;
    if (!shared) return;
    if (frames > HUSH_SPEAKER_CAPACITY) { // Only the newest capacity frames fit.
        in += (size_t)(frames - HUSH_SPEAKER_CAPACITY) * HUSH_SPEAKER_CHANNELS;
        frames = HUSH_SPEAKER_CAPACITY;
    }
    uint64_t w = atomic_load_explicit((_Atomic uint64_t *)&shared->write_pos, memory_order_relaxed);
    for (UInt32 i = 0; i < frames; i++) {
        float v = 0.5f * (in[2 * i] + in[2 * i + 1]);
        v = v != v ? 0.0f : v > 1.0f ? 1.0f : v < -1.0f ? -1.0f : v;
        shared->samples[(w + i) & (HUSH_SPEAKER_CAPACITY - 1)] = v;
    }
    atomic_store_explicit((_Atomic uint64_t *)&shared->write_host_time, mach_absolute_time(), memory_order_relaxed);
    atomic_store_explicit((_Atomic uint32_t *)&shared->write_frames, frames, memory_order_relaxed);
    atomic_store_explicit((_Atomic uint64_t *)&shared->write_pos, w + frames, memory_order_release);
    gSpeakerWrites++;
    gSpeakerFrames += frames;
}

// Copy the audio for device time [time, time + frames) to `out` (rules in ../ring.h).
// Every client that reads the same device time gets the same samples. Real-time safe.
static void ReadShared(float *out, UInt32 frames, Float64 time) {
    HushMicShared *shared = gShared;
    if (!shared) {
        memset(out, 0, frames * sizeof(float));
        return;
    }
    gReads++;
    if (gLastTime >= 0 && time != gLastTime && time != gLastTime + frames) gTimeJumps++;
    gLastTime = time;
    int64_t write = (int64_t)atomic_load_explicit((_Atomic uint64_t *)&shared->write_pos, memory_order_acquire);
    int64_t end = hush_ring_read(&gReader, shared->samples, HUSH_MIC_CAPACITY, write, (int64_t)time, out, frames);
    // Publish the furthest position read; the app keeps the ring filled beyond it.
    uint64_t read = atomic_load_explicit((_Atomic uint64_t *)&shared->read_pos, memory_order_relaxed);
    if (end > 0 && ((uint64_t)end > read || read - (uint64_t)end > HUSH_MIC_CAPACITY)) {
        atomic_store_explicit((_Atomic uint32_t *)&shared->read_frames, frames, memory_order_relaxed);
        atomic_store_explicit((_Atomic uint64_t *)&shared->read_pos, (uint64_t)end, memory_order_release);
    }
}

// ---------------------------------------------------------------------------------------------
// Helpers for property data

#define RETURN_IF(cond, err) do { if (cond) return (err); } while (0)

static OSStatus PutU32(UInt32 value, UInt32 size, UInt32 *outSize, void *out) {
    RETURN_IF(size < sizeof(UInt32), kAudioHardwareBadPropertySizeError);
    *(UInt32 *)out = value;
    *outSize = sizeof(UInt32);
    return 0;
}

static OSStatus PutF64(Float64 value, UInt32 size, UInt32 *outSize, void *out) {
    RETURN_IF(size < sizeof(Float64), kAudioHardwareBadPropertySizeError);
    *(Float64 *)out = value;
    *outSize = sizeof(Float64);
    return 0;
}

static OSStatus PutString(const char *value, UInt32 size, UInt32 *outSize, void *out) {
    RETURN_IF(size < sizeof(CFStringRef), kAudioHardwareBadPropertySizeError);
    // The caller releases the string.
    *(CFStringRef *)out = CFStringCreateWithCString(NULL, value, kCFStringEncodingUTF8);
    *outSize = sizeof(CFStringRef);
    return 0;
}

static OSStatus PutObjects(const AudioObjectID *ids, UInt32 count, UInt32 size, UInt32 *outSize, void *out) {
    UInt32 n = size / sizeof(AudioObjectID);
    if (n > count) n = count;
    memcpy(out, ids, n * sizeof(AudioObjectID));
    *outSize = n * sizeof(AudioObjectID);
    return 0;
}

// 48 kHz float32 with 1 channel (microphone) or 2 (speaker).
static AudioStreamBasicDescription Format(UInt32 channels) {
    AudioStreamBasicDescription f = {0};
    f.mSampleRate = HUSH_MIC_RATE;
    f.mFormatID = kAudioFormatLinearPCM;
    f.mFormatFlags = kAudioFormatFlagIsFloat | kAudioFormatFlagsNativeEndian | kAudioFormatFlagIsPacked;
    f.mBytesPerPacket = channels * sizeof(float);
    f.mFramesPerPacket = 1;
    f.mBytesPerFrame = channels * sizeof(float);
    f.mChannelsPerFrame = channels;
    f.mBitsPerChannel = 32;
    return f;
}

static UInt32 StreamChannels(AudioObjectID stream) {
    return stream == kObjectID_SpeakerStream ? HUSH_SPEAKER_CHANNELS : 1;
}

static bool InputScope(const AudioObjectPropertyAddress *a) {
    return a->mScope == kAudioObjectPropertyScopeGlobal || a->mScope == kAudioObjectPropertyScopeInput;
}

static bool OutputScope(const AudioObjectPropertyAddress *a) {
    return a->mScope == kAudioObjectPropertyScopeGlobal || a->mScope == kAudioObjectPropertyScopeOutput;
}

static DeviceClock *ClockOf(AudioObjectID dev) {
    return dev == kObjectID_Device ? &gMicClock : dev == kObjectID_SpeakerDevice ? &gSpeakerClock : NULL;
}

// ---------------------------------------------------------------------------------------------
// Properties

static Boolean HasProperty(AudioServerPlugInDriverRef d, AudioObjectID obj, pid_t pid,
                           const AudioObjectPropertyAddress *a) {
    (void)d; (void)pid;
    switch (obj) {
    case kObjectID_PlugIn:
        switch (a->mSelector) {
        case kAudioObjectPropertyBaseClass: case kAudioObjectPropertyClass:
        case kAudioObjectPropertyOwner: case kAudioObjectPropertyManufacturer:
        case kAudioObjectPropertyOwnedObjects: case kAudioPlugInPropertyDeviceList:
        case kAudioPlugInPropertyTranslateUIDToDevice: case kAudioPlugInPropertyResourceBundle:
            return true;
        }
        return false;
    case kObjectID_Device:
    case kObjectID_SpeakerDevice:
        switch (a->mSelector) {
        case kAudioObjectPropertyBaseClass: case kAudioObjectPropertyClass:
        case kAudioObjectPropertyOwner: case kAudioObjectPropertyName:
        case kAudioObjectPropertyManufacturer: case kAudioObjectPropertyOwnedObjects:
        case kAudioDevicePropertyDeviceUID: case kAudioDevicePropertyModelUID:
        case kAudioDevicePropertyTransportType: case kAudioDevicePropertyRelatedDevices:
        case kAudioDevicePropertyClockDomain: case kAudioDevicePropertyDeviceIsAlive:
        case kAudioDevicePropertyDeviceIsRunning: case kAudioObjectPropertyControlList:
        case kAudioDevicePropertyNominalSampleRate: case kAudioDevicePropertyAvailableNominalSampleRates:
        case kAudioDevicePropertyIsHidden: case kAudioDevicePropertyZeroTimeStampPeriod:
        case kAudioDevicePropertyStreams:
            return true;
        case kAudioDevicePropertyDeviceCanBeDefaultDevice:
        case kAudioDevicePropertyDeviceCanBeDefaultSystemDevice:
        case kAudioDevicePropertyLatency: case kAudioDevicePropertySafetyOffset:
        case kAudioDevicePropertyPreferredChannelsForStereo:
        case kAudioDevicePropertyPreferredChannelLayout:
            return obj == kObjectID_Device ? InputScope(a) : OutputScope(a);
        }
        return false;
    case kObjectID_Stream:
    case kObjectID_SpeakerStream:
        switch (a->mSelector) {
        case kAudioObjectPropertyBaseClass: case kAudioObjectPropertyClass:
        case kAudioObjectPropertyOwner: case kAudioObjectPropertyOwnedObjects:
        case kAudioStreamPropertyIsActive: case kAudioStreamPropertyDirection:
        case kAudioStreamPropertyTerminalType: case kAudioStreamPropertyStartingChannel:
        case kAudioStreamPropertyLatency: case kAudioStreamPropertyVirtualFormat:
        case kAudioStreamPropertyPhysicalFormat: case kAudioStreamPropertyAvailableVirtualFormats:
        case kAudioStreamPropertyAvailablePhysicalFormats:
            return true;
        }
        return false;
    }
    return false;
}

static OSStatus IsPropertySettable(AudioServerPlugInDriverRef d, AudioObjectID obj, pid_t pid,
                                   const AudioObjectPropertyAddress *a, Boolean *out) {
    RETURN_IF(!HasProperty(d, obj, pid, a), kAudioHardwareUnknownPropertyError);
    // Rate and format are fixed. Accepting a write of the current value keeps clients happy.
    *out = ((obj == kObjectID_Device || obj == kObjectID_SpeakerDevice) &&
            a->mSelector == kAudioDevicePropertyNominalSampleRate) ||
           ((obj == kObjectID_Stream || obj == kObjectID_SpeakerStream) && (a->mSelector == kAudioStreamPropertyVirtualFormat ||
                                        a->mSelector == kAudioStreamPropertyPhysicalFormat ||
                                        a->mSelector == kAudioStreamPropertyIsActive));
    return 0;
}

static OSStatus GetPropertyData(AudioServerPlugInDriverRef d, AudioObjectID obj, pid_t pid,
                                const AudioObjectPropertyAddress *a, UInt32 qSize, const void *q,
                                UInt32 size, UInt32 *outSize, void *out);

static OSStatus GetPropertyDataSize(AudioServerPlugInDriverRef d, AudioObjectID obj, pid_t pid,
                                    const AudioObjectPropertyAddress *a, UInt32 qSize, const void *q,
                                    UInt32 *outSize) {
    RETURN_IF(!HasProperty(d, obj, pid, a), kAudioHardwareUnknownPropertyError);
    // Every property here fits in this buffer; ask for the data and report its size.
    union { AudioStreamRangedDescription r; AudioValueRange v; UInt32 u[8]; Float64 f; CFStringRef s;
            AudioChannelLayout l; } buffer;
    switch (a->mSelector) {
    case kAudioObjectPropertyName: case kAudioObjectPropertyManufacturer:
    case kAudioDevicePropertyDeviceUID: case kAudioDevicePropertyModelUID:
    case kAudioPlugInPropertyResourceBundle:
        *outSize = sizeof(CFStringRef); // Do not create a string just to size it.
        return 0;
    }
    return GetPropertyData(d, obj, pid, a, qSize, q, sizeof(buffer), outSize, &buffer);
}

static OSStatus GetPropertyData(AudioServerPlugInDriverRef d, AudioObjectID obj, pid_t pid,
                                const AudioObjectPropertyAddress *a, UInt32 qSize, const void *q,
                                UInt32 size, UInt32 *outSize, void *out) {
    (void)d; (void)pid;
    *outSize = 0;
    switch (obj) {
    case kObjectID_PlugIn:
        switch (a->mSelector) {
        case kAudioObjectPropertyBaseClass: return PutU32(kAudioObjectClassID, size, outSize, out);
        case kAudioObjectPropertyClass: return PutU32(kAudioPlugInClassID, size, outSize, out);
        case kAudioObjectPropertyOwner: return PutU32(kAudioObjectUnknown, size, outSize, out);
        case kAudioObjectPropertyManufacturer: return PutString(kManufacturer, size, outSize, out);
        case kAudioObjectPropertyOwnedObjects:
        case kAudioPlugInPropertyDeviceList: {
            AudioObjectID ids[] = {kObjectID_Device, kObjectID_SpeakerDevice};
            return PutObjects(ids, 2, size, outSize, out);
        }
        case kAudioPlugInPropertyTranslateUIDToDevice: {
            RETURN_IF(qSize != sizeof(CFStringRef) || !q, kAudioHardwareBadPropertySizeError);
            const char *uids[] = {kDevice_UID, kSpeaker_UID};
            AudioObjectID devices[] = {kObjectID_Device, kObjectID_SpeakerDevice};
            AudioObjectID found = kAudioObjectUnknown;
            for (int i = 0; i < 2; i++) {
                CFStringRef uid = CFStringCreateWithCString(NULL, uids[i], kCFStringEncodingUTF8);
                if (CFStringCompare(*(const CFStringRef *)q, uid, 0) == kCFCompareEqualTo) found = devices[i];
                CFRelease(uid);
            }
            return PutU32(found, size, outSize, out);
        }
        case kAudioPlugInPropertyResourceBundle: return PutString("", size, outSize, out);
        }
        break;
    case kObjectID_Device:
    case kObjectID_SpeakerDevice: {
        bool speaker = obj == kObjectID_SpeakerDevice;
        switch (a->mSelector) {
        case kAudioObjectPropertyBaseClass: return PutU32(kAudioObjectClassID, size, outSize, out);
        case kAudioObjectPropertyClass: return PutU32(kAudioDeviceClassID, size, outSize, out);
        case kAudioObjectPropertyOwner: return PutU32(kObjectID_PlugIn, size, outSize, out);
        case kAudioObjectPropertyName: return PutString(speaker ? kSpeaker_Name : kDevice_Name, size, outSize, out);
        case kAudioObjectPropertyManufacturer: return PutString(kManufacturer, size, outSize, out);
        case kAudioDevicePropertyDeviceUID: return PutString(speaker ? kSpeaker_UID : kDevice_UID, size, outSize, out);
        case kAudioDevicePropertyModelUID:
            return PutString(speaker ? kSpeaker_ModelUID : kDevice_ModelUID, size, outSize, out);
        case kAudioDevicePropertyTransportType: return PutU32(kAudioDeviceTransportTypeVirtual, size, outSize, out);
        case kAudioDevicePropertyClockDomain: return PutU32(0, size, outSize, out);
        case kAudioDevicePropertyDeviceIsAlive: return PutU32(1, size, outSize, out);
        case kAudioDevicePropertyDeviceIsRunning: {
            pthread_mutex_lock(&gStateMutex);
            UInt32 running = ClockOf(obj)->ioCount > 0;
            pthread_mutex_unlock(&gStateMutex);
            return PutU32(running, size, outSize, out);
        }
        case kAudioDevicePropertyDeviceCanBeDefaultDevice:
            // Selectable as the default in its direction. Never the system (alert) device.
            return PutU32(a->mScope == (speaker ? kAudioObjectPropertyScopeOutput : kAudioObjectPropertyScopeInput),
                          size, outSize, out);
        case kAudioDevicePropertyDeviceCanBeDefaultSystemDevice: return PutU32(0, size, outSize, out);
        case kAudioDevicePropertyLatency: case kAudioDevicePropertySafetyOffset:
            return PutU32(0, size, outSize, out);
        case kAudioDevicePropertyIsHidden: return PutU32(0, size, outSize, out);
        case kAudioDevicePropertyZeroTimeStampPeriod: return PutU32(kZeroTimeStampPeriod, size, outSize, out);
        case kAudioDevicePropertyNominalSampleRate: return PutF64(HUSH_MIC_RATE, size, outSize, out);
        case kAudioDevicePropertyAvailableNominalSampleRates: {
            RETURN_IF(size < sizeof(AudioValueRange), kAudioHardwareBadPropertySizeError);
            ((AudioValueRange *)out)->mMinimum = HUSH_MIC_RATE;
            ((AudioValueRange *)out)->mMaximum = HUSH_MIC_RATE;
            *outSize = sizeof(AudioValueRange);
            return 0;
        }
        case kAudioObjectPropertyOwnedObjects:
        case kAudioDevicePropertyStreams: {
            if (speaker) {
                AudioObjectID ids[] = {kObjectID_SpeakerStream};
                return PutObjects(ids, a->mScope == kAudioObjectPropertyScopeInput ? 0 : 1, size, outSize, out);
            }
            AudioObjectID ids[] = {kObjectID_Stream};
            bool output = a->mScope == kAudioObjectPropertyScopeOutput;
            return PutObjects(ids, output ? 0 : 1, size, outSize, out);
        }
        case kAudioDevicePropertyRelatedDevices: {
            AudioObjectID ids[] = {obj};
            return PutObjects(ids, 1, size, outSize, out);
        }
        case kAudioObjectPropertyControlList: return PutObjects(NULL, 0, size, outSize, out);
        case kAudioDevicePropertyPreferredChannelsForStereo: {
            RETURN_IF(size < 2 * sizeof(UInt32), kAudioHardwareBadPropertySizeError);
            ((UInt32 *)out)[0] = 1;
            ((UInt32 *)out)[1] = speaker ? 2 : 1;
            *outSize = 2 * sizeof(UInt32);
            return 0;
        }
        case kAudioDevicePropertyPreferredChannelLayout: {
            RETURN_IF(size < sizeof(AudioChannelLayout), kAudioHardwareBadPropertySizeError);
            AudioChannelLayout *layout = out;
            memset(layout, 0, sizeof(*layout));
            layout->mChannelLayoutTag = speaker ? kAudioChannelLayoutTag_Stereo : kAudioChannelLayoutTag_Mono;
            *outSize = sizeof(AudioChannelLayout);
            return 0;
        }
        }
        break;
    }
    case kObjectID_Stream:
    case kObjectID_SpeakerStream: {
        bool speaker = obj == kObjectID_SpeakerStream;
        switch (a->mSelector) {
        case kAudioObjectPropertyBaseClass: return PutU32(kAudioObjectClassID, size, outSize, out);
        case kAudioObjectPropertyClass: return PutU32(kAudioStreamClassID, size, outSize, out);
        case kAudioObjectPropertyOwner:
            return PutU32(speaker ? kObjectID_SpeakerDevice : kObjectID_Device, size, outSize, out);
        case kAudioObjectPropertyOwnedObjects: return PutObjects(NULL, 0, size, outSize, out);
        case kAudioStreamPropertyIsActive: return PutU32(1, size, outSize, out);
        case kAudioStreamPropertyDirection: return PutU32(speaker ? 0 : 1, size, outSize, out); // 1 = input
        case kAudioStreamPropertyTerminalType:
            return PutU32(speaker ? kAudioStreamTerminalTypeSpeaker : kAudioStreamTerminalTypeMicrophone,
                          size, outSize, out);
        case kAudioStreamPropertyStartingChannel: return PutU32(1, size, outSize, out);
        case kAudioStreamPropertyLatency: return PutU32(0, size, outSize, out);
        case kAudioStreamPropertyVirtualFormat: case kAudioStreamPropertyPhysicalFormat: {
            RETURN_IF(size < sizeof(AudioStreamBasicDescription), kAudioHardwareBadPropertySizeError);
            *(AudioStreamBasicDescription *)out = Format(StreamChannels(obj));
            *outSize = sizeof(AudioStreamBasicDescription);
            return 0;
        }
        case kAudioStreamPropertyAvailableVirtualFormats:
        case kAudioStreamPropertyAvailablePhysicalFormats: {
            RETURN_IF(size < sizeof(AudioStreamRangedDescription), kAudioHardwareBadPropertySizeError);
            AudioStreamRangedDescription *r = out;
            r->mFormat = Format(StreamChannels(obj));
            r->mSampleRateRange.mMinimum = HUSH_MIC_RATE;
            r->mSampleRateRange.mMaximum = HUSH_MIC_RATE;
            *outSize = sizeof(AudioStreamRangedDescription);
            return 0;
        }
        }
        break;
    }
    }
    return kAudioHardwareUnknownPropertyError;
}

static OSStatus SetPropertyData(AudioServerPlugInDriverRef d, AudioObjectID obj, pid_t pid,
                                const AudioObjectPropertyAddress *a, UInt32 qSize, const void *q,
                                UInt32 size, const void *data) {
    (void)d; (void)pid; (void)qSize; (void)q;
    bool stream = obj == kObjectID_Stream || obj == kObjectID_SpeakerStream;
    if ((obj == kObjectID_Device || obj == kObjectID_SpeakerDevice) &&
        a->mSelector == kAudioDevicePropertyNominalSampleRate) {
        RETURN_IF(size != sizeof(Float64), kAudioHardwareBadPropertySizeError);
        return *(const Float64 *)data == HUSH_MIC_RATE ? 0 : kAudioHardwareIllegalOperationError;
    }
    if (stream && (a->mSelector == kAudioStreamPropertyVirtualFormat ||
                   a->mSelector == kAudioStreamPropertyPhysicalFormat)) {
        RETURN_IF(size != sizeof(AudioStreamBasicDescription), kAudioHardwareBadPropertySizeError);
        AudioStreamBasicDescription want = Format(StreamChannels(obj)), got = *(const AudioStreamBasicDescription *)data;
        return got.mSampleRate == want.mSampleRate && got.mFormatID == want.mFormatID &&
               got.mChannelsPerFrame == want.mChannelsPerFrame && got.mBitsPerChannel == want.mBitsPerChannel
                   ? 0 : kAudioDeviceUnsupportedFormatError;
    }
    if (stream && a->mSelector == kAudioStreamPropertyIsActive) return 0;
    return kAudioHardwareUnknownPropertyError;
}

// ---------------------------------------------------------------------------------------------
// IO

static OSStatus StartIO(AudioServerPlugInDriverRef d, AudioObjectID dev, UInt32 client) {
    (void)d; (void)client;
    DeviceClock *clock = ClockOf(dev);
    RETURN_IF(!clock, kAudioHardwareBadObjectError);
    pthread_mutex_lock(&gStateMutex);
    if (clock->ioCount++ == 0) {
        clock->timeStampCount = 0;
        clock->anchorHostTime = mach_absolute_time();
        if (dev == kObjectID_Device) {
            OpenShared(); // Retried at each start, outside the IO path.
            gReader.synced = 0;
            os_log(gLog, "start: shared memory %{public}s", gShared ? "mapped" : "missing");
        } else {
            OpenSpeakerShared();
            os_log(gLog, "speaker start: shared memory %{public}s", gSpeakerShared ? "mapped" : "missing");
        }
    }
    pthread_mutex_unlock(&gStateMutex);
    return 0;
}

static OSStatus StopIO(AudioServerPlugInDriverRef d, AudioObjectID dev, UInt32 client) {
    (void)d; (void)client;
    DeviceClock *clock = ClockOf(dev);
    RETURN_IF(!clock, kAudioHardwareBadObjectError);
    pthread_mutex_lock(&gStateMutex);
    if (clock->ioCount > 0) clock->ioCount--;
    if (clock->ioCount == 0 && dev == kObjectID_Device) {
        os_log(gLog, "stop: reads %llu, resyncs %llu, missing frames %llu, time jumps %llu",
               gReads, gReader.resyncs, gReader.missing, gTimeJumps);
        gReads = gTimeJumps = gReader.resyncs = gReader.missing = 0;
        gLastTime = -1;
    } else if (clock->ioCount == 0) {
        os_log(gLog, "speaker stop: writes %llu, frames %llu", gSpeakerWrites, gSpeakerFrames);
        gSpeakerWrites = gSpeakerFrames = 0;
    }
    pthread_mutex_unlock(&gStateMutex);
    return 0;
}

static OSStatus GetZeroTimeStamp(AudioServerPlugInDriverRef d, AudioObjectID dev, UInt32 client,
                                 Float64 *sampleTime, UInt64 *hostTime, UInt64 *seed) {
    (void)d; (void)client;
    DeviceClock *clock = ClockOf(dev);
    RETURN_IF(!clock, kAudioHardwareBadObjectError);
    // A free-running 48 kHz clock on the host clock, as in NullAudio. Called on the IO thread.
    UInt64 now = mach_absolute_time();
    Float64 ticksPerPeriod = gHostTicksPerFrame * kZeroTimeStampPeriod;
    if (clock->anchorHostTime + (UInt64)((clock->timeStampCount + 1) * ticksPerPeriod) <= now)
        clock->timeStampCount++;
    *sampleTime = (Float64)(clock->timeStampCount * kZeroTimeStampPeriod);
    *hostTime = clock->anchorHostTime + (UInt64)(clock->timeStampCount * ticksPerPeriod);
    *seed = 1;
    return 0;
}

static OSStatus WillDoIOOperation(AudioServerPlugInDriverRef d, AudioObjectID dev, UInt32 client,
                                  UInt32 op, Boolean *willDo, Boolean *inPlace) {
    (void)d; (void)client;
    RETURN_IF(!ClockOf(dev), kAudioHardwareBadObjectError);
    *willDo = op == (dev == kObjectID_Device ? kAudioServerPlugInIOOperationReadInput
                                             : kAudioServerPlugInIOOperationWriteMix);
    *inPlace = true;
    return 0;
}

static OSStatus BeginIOOperation(AudioServerPlugInDriverRef d, AudioObjectID dev, UInt32 client, UInt32 op,
                                 UInt32 frames, const AudioServerPlugInIOCycleInfo *cycle) {
    (void)d; (void)client; (void)op; (void)frames; (void)cycle;
    return ClockOf(dev) ? 0 : kAudioHardwareBadObjectError;
}

static OSStatus DoIOOperation(AudioServerPlugInDriverRef d, AudioObjectID dev, AudioObjectID stream,
                              UInt32 client, UInt32 op, UInt32 frames, const AudioServerPlugInIOCycleInfo *cycle,
                              void *main, void *secondary) {
    (void)d; (void)client; (void)cycle; (void)secondary;
    if (dev == kObjectID_SpeakerDevice && stream == kObjectID_SpeakerStream) {
        if (op == kAudioServerPlugInIOOperationWriteMix && main) WriteSpeaker(main, frames);
        return 0;
    }
    RETURN_IF(dev != kObjectID_Device || stream != kObjectID_Stream, kAudioHardwareBadObjectError);
    if (op == kAudioServerPlugInIOOperationReadInput && main) ReadShared(main, frames, cycle->mInputTime.mSampleTime);
    return 0;
}

static OSStatus EndIOOperation(AudioServerPlugInDriverRef d, AudioObjectID dev, UInt32 client, UInt32 op,
                               UInt32 frames, const AudioServerPlugInIOCycleInfo *cycle) {
    (void)d; (void)client; (void)op; (void)frames; (void)cycle;
    return ClockOf(dev) ? 0 : kAudioHardwareBadObjectError;
}

// ---------------------------------------------------------------------------------------------
// Plug-in lifetime

static HRESULT QueryInterface(void *d, REFIID uuid, LPVOID *out);
static ULONG AddRef(void *d) {
    (void)d;
    pthread_mutex_lock(&gStateMutex);
    ULONG n = ++gRefCount;
    pthread_mutex_unlock(&gStateMutex);
    return n;
}
static ULONG Release(void *d) {
    (void)d;
    pthread_mutex_lock(&gStateMutex);
    ULONG n = gRefCount > 0 ? --gRefCount : 0;
    pthread_mutex_unlock(&gStateMutex);
    return n;
}

static OSStatus Initialize(AudioServerPlugInDriverRef d, AudioServerPlugInHostRef host) {
    (void)d;
    gHost = host;
    mach_timebase_info_data_t timebase;
    mach_timebase_info(&timebase);
    Float64 hostClockHz = 1e9 * (Float64)timebase.denom / (Float64)timebase.numer;
    gHostTicksPerFrame = hostClockHz / HUSH_MIC_RATE;
    pthread_mutex_lock(&gStateMutex);
    OpenShared();
    OpenSpeakerShared();
    pthread_mutex_unlock(&gStateMutex);
    return 0;
}

static OSStatus CreateDevice(AudioServerPlugInDriverRef d, CFDictionaryRef desc,
                             const AudioServerPlugInClientInfo *client, AudioObjectID *out) {
    (void)d; (void)desc; (void)client; (void)out;
    return kAudioHardwareUnsupportedOperationError;
}
static OSStatus DestroyDevice(AudioServerPlugInDriverRef d, AudioObjectID dev) {
    (void)d; (void)dev;
    return kAudioHardwareUnsupportedOperationError;
}
static OSStatus AddDeviceClient(AudioServerPlugInDriverRef d, AudioObjectID dev,
                                const AudioServerPlugInClientInfo *client) {
    (void)d; (void)client;
    return ClockOf(dev) ? 0 : kAudioHardwareBadObjectError;
}
static OSStatus RemoveDeviceClient(AudioServerPlugInDriverRef d, AudioObjectID dev,
                                   const AudioServerPlugInClientInfo *client) {
    (void)d; (void)client;
    return ClockOf(dev) ? 0 : kAudioHardwareBadObjectError;
}
static OSStatus PerformDeviceConfigurationChange(AudioServerPlugInDriverRef d, AudioObjectID dev,
                                                 UInt64 action, void *info) {
    (void)d; (void)action; (void)info;
    return ClockOf(dev) ? 0 : kAudioHardwareBadObjectError;
}
static OSStatus AbortDeviceConfigurationChange(AudioServerPlugInDriverRef d, AudioObjectID dev,
                                               UInt64 action, void *info) {
    (void)d; (void)action; (void)info;
    return ClockOf(dev) ? 0 : kAudioHardwareBadObjectError;
}

static AudioServerPlugInDriverInterface gInterface = {
    NULL, QueryInterface, AddRef, Release, Initialize, CreateDevice, DestroyDevice,
    AddDeviceClient, RemoveDeviceClient, PerformDeviceConfigurationChange,
    AbortDeviceConfigurationChange, HasProperty, IsPropertySettable, GetPropertyDataSize,
    GetPropertyData, SetPropertyData, StartIO, StopIO, GetZeroTimeStamp, WillDoIOOperation,
    BeginIOOperation, DoIOOperation, EndIOOperation,
};
static AudioServerPlugInDriverInterface *gInterfacePtr = &gInterface;
static AudioServerPlugInDriverRef gDriverRef = &gInterfacePtr;

static HRESULT QueryInterface(void *d, REFIID uuid, LPVOID *out) {
    RETURN_IF(d != gDriverRef || !out, kAudioHardwareBadObjectError);
    CFUUIDRef requested = CFUUIDCreateFromUUIDBytes(NULL, uuid);
    bool known = CFEqual(requested, IUnknownUUID) || CFEqual(requested, kAudioServerPlugInDriverInterfaceUUID);
    CFRelease(requested);
    if (!known) return E_NOINTERFACE;
    AddRef(d);
    *out = gDriverRef;
    return S_OK;
}

// Factory named in Info.plist.
void *HushMic_Create(CFAllocatorRef allocator, CFUUIDRef type) {
    (void)allocator;
    if (!CFEqual(type, kAudioServerPlugInTypeUUID)) return NULL;
    gLog = os_log_create("com.hush.mic", "driver");
    return gDriverRef;
}
