// Hush AI Microphone: a minimal Core Audio server plug-in with one virtual input device.
//
// Structure follows Apple's NullAudio sample ("Creating an Audio Server Driver Plug-in"):
// one plug-in object, one device, one input stream, fixed 48 kHz mono float32, no controls.
// Audio comes from the Hush app through the shared-memory ring in ../shared.h.
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
    kObjectID_Device = 2,
    kObjectID_Stream = 3,
};

#define kDevice_Name "Hush AI Microphone"
#define kDevice_UID "HushAIMicrophone_UID"
#define kDevice_ModelUID "HushAIMicrophone_ModelUID"
#define kManufacturer "Hush"
#define kZeroTimeStampPeriod 16384u


static pthread_mutex_t gStateMutex = PTHREAD_MUTEX_INITIALIZER;
static AudioServerPlugInHostRef gHost = NULL;
static UInt32 gRefCount = 0;
static UInt32 gIOCount = 0;
static Float64 gHostTicksPerFrame = 0;
static UInt64 gAnchorHostTime = 0;
static UInt64 gTimeStampCount = 0;
static HushMicShared *gShared = NULL;
/* Reader state (../ring.h). Used on the IO thread; reset when IO starts. */
static HushRingReader gReader;
/* Diagnostics, written on the IO thread and logged at stop. */
static UInt64 gTimeJumps, gReads;
static Float64 gLastTime = -1;
static os_log_t gLog;

// ---------------------------------------------------------------------------------------------
// Shared memory

static void OpenShared(void) {
    if (gShared) return;
    int fd = shm_open(HUSH_MIC_SHM_NAME, O_RDWR | O_CREAT, 0666);
    if (fd < 0) {
        os_log_error(gLog, "shm_open failed: %{errno}d", errno);
        return;
    }
    fchmod(fd, 0666); // The app runs as the user, the driver as _coreaudiod.
    // macOS rounds the size up to whole pages and allows setting it only once.
    struct stat st;
    if (fstat(fd, &st) != 0 || (st.st_size < (off_t)sizeof(HushMicShared) && ftruncate(fd, sizeof(HushMicShared)) != 0)) {
        os_log_error(gLog, "shared memory size failed: %{errno}d", errno);
        close(fd);
        return;
    }
    void *map = mmap(NULL, sizeof(HushMicShared), PROT_READ | PROT_WRITE, MAP_SHARED, fd, 0);
    close(fd);
    if (map == MAP_FAILED) {
        os_log_error(gLog, "mmap failed: %{errno}d", errno);
        return;
    }
    HushMicShared *shared = map;
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

static AudioStreamBasicDescription Format(void) {
    AudioStreamBasicDescription f = {0};
    f.mSampleRate = HUSH_MIC_RATE;
    f.mFormatID = kAudioFormatLinearPCM;
    f.mFormatFlags = kAudioFormatFlagIsFloat | kAudioFormatFlagsNativeEndian | kAudioFormatFlagIsPacked;
    f.mBytesPerPacket = sizeof(float);
    f.mFramesPerPacket = 1;
    f.mBytesPerFrame = sizeof(float);
    f.mChannelsPerFrame = 1;
    f.mBitsPerChannel = 32;
    return f;
}

static bool InputScope(const AudioObjectPropertyAddress *a) {
    return a->mScope == kAudioObjectPropertyScopeGlobal || a->mScope == kAudioObjectPropertyScopeInput;
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
            return InputScope(a);
        }
        return false;
    case kObjectID_Stream:
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
    *out = (obj == kObjectID_Device && a->mSelector == kAudioDevicePropertyNominalSampleRate) ||
           (obj == kObjectID_Stream && (a->mSelector == kAudioStreamPropertyVirtualFormat ||
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
            AudioObjectID ids[] = {kObjectID_Device};
            return PutObjects(ids, 1, size, outSize, out);
        }
        case kAudioPlugInPropertyTranslateUIDToDevice: {
            RETURN_IF(qSize != sizeof(CFStringRef) || !q, kAudioHardwareBadPropertySizeError);
            CFStringRef uid = CFStringCreateWithCString(NULL, kDevice_UID, kCFStringEncodingUTF8);
            bool match = CFStringCompare(*(const CFStringRef *)q, uid, 0) == kCFCompareEqualTo;
            CFRelease(uid);
            return PutU32(match ? kObjectID_Device : kAudioObjectUnknown, size, outSize, out);
        }
        case kAudioPlugInPropertyResourceBundle: return PutString("", size, outSize, out);
        }
        break;
    case kObjectID_Device:
        switch (a->mSelector) {
        case kAudioObjectPropertyBaseClass: return PutU32(kAudioObjectClassID, size, outSize, out);
        case kAudioObjectPropertyClass: return PutU32(kAudioDeviceClassID, size, outSize, out);
        case kAudioObjectPropertyOwner: return PutU32(kObjectID_PlugIn, size, outSize, out);
        case kAudioObjectPropertyName: return PutString(kDevice_Name, size, outSize, out);
        case kAudioObjectPropertyManufacturer: return PutString(kManufacturer, size, outSize, out);
        case kAudioDevicePropertyDeviceUID: return PutString(kDevice_UID, size, outSize, out);
        case kAudioDevicePropertyModelUID: return PutString(kDevice_ModelUID, size, outSize, out);
        case kAudioDevicePropertyTransportType: return PutU32(kAudioDeviceTransportTypeVirtual, size, outSize, out);
        case kAudioDevicePropertyClockDomain: return PutU32(0, size, outSize, out);
        case kAudioDevicePropertyDeviceIsAlive: return PutU32(1, size, outSize, out);
        case kAudioDevicePropertyDeviceIsRunning: {
            pthread_mutex_lock(&gStateMutex);
            UInt32 running = gIOCount > 0;
            pthread_mutex_unlock(&gStateMutex);
            return PutU32(running, size, outSize, out);
        }
        case kAudioDevicePropertyDeviceCanBeDefaultDevice:
            return PutU32(a->mScope == kAudioObjectPropertyScopeInput, size, outSize, out);
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
            AudioObjectID ids[] = {kObjectID_Stream};
            bool output = a->mScope == kAudioObjectPropertyScopeOutput;
            return PutObjects(ids, output ? 0 : 1, size, outSize, out);
        }
        case kAudioDevicePropertyRelatedDevices: {
            AudioObjectID ids[] = {kObjectID_Device};
            return PutObjects(ids, 1, size, outSize, out);
        }
        case kAudioObjectPropertyControlList: return PutObjects(NULL, 0, size, outSize, out);
        case kAudioDevicePropertyPreferredChannelsForStereo: {
            RETURN_IF(size < 2 * sizeof(UInt32), kAudioHardwareBadPropertySizeError);
            ((UInt32 *)out)[0] = 1;
            ((UInt32 *)out)[1] = 1;
            *outSize = 2 * sizeof(UInt32);
            return 0;
        }
        case kAudioDevicePropertyPreferredChannelLayout: {
            RETURN_IF(size < sizeof(AudioChannelLayout), kAudioHardwareBadPropertySizeError);
            AudioChannelLayout *layout = out;
            memset(layout, 0, sizeof(*layout));
            layout->mChannelLayoutTag = kAudioChannelLayoutTag_Mono;
            *outSize = sizeof(AudioChannelLayout);
            return 0;
        }
        }
        break;
    case kObjectID_Stream:
        switch (a->mSelector) {
        case kAudioObjectPropertyBaseClass: return PutU32(kAudioObjectClassID, size, outSize, out);
        case kAudioObjectPropertyClass: return PutU32(kAudioStreamClassID, size, outSize, out);
        case kAudioObjectPropertyOwner: return PutU32(kObjectID_Device, size, outSize, out);
        case kAudioObjectPropertyOwnedObjects: return PutObjects(NULL, 0, size, outSize, out);
        case kAudioStreamPropertyIsActive: return PutU32(1, size, outSize, out);
        case kAudioStreamPropertyDirection: return PutU32(1, size, outSize, out); // 1 = input
        case kAudioStreamPropertyTerminalType: return PutU32(kAudioStreamTerminalTypeMicrophone, size, outSize, out);
        case kAudioStreamPropertyStartingChannel: return PutU32(1, size, outSize, out);
        case kAudioStreamPropertyLatency: return PutU32(0, size, outSize, out);
        case kAudioStreamPropertyVirtualFormat: case kAudioStreamPropertyPhysicalFormat: {
            RETURN_IF(size < sizeof(AudioStreamBasicDescription), kAudioHardwareBadPropertySizeError);
            *(AudioStreamBasicDescription *)out = Format();
            *outSize = sizeof(AudioStreamBasicDescription);
            return 0;
        }
        case kAudioStreamPropertyAvailableVirtualFormats:
        case kAudioStreamPropertyAvailablePhysicalFormats: {
            RETURN_IF(size < sizeof(AudioStreamRangedDescription), kAudioHardwareBadPropertySizeError);
            AudioStreamRangedDescription *r = out;
            r->mFormat = Format();
            r->mSampleRateRange.mMinimum = HUSH_MIC_RATE;
            r->mSampleRateRange.mMaximum = HUSH_MIC_RATE;
            *outSize = sizeof(AudioStreamRangedDescription);
            return 0;
        }
        }
        break;
    }
    return kAudioHardwareUnknownPropertyError;
}

static OSStatus SetPropertyData(AudioServerPlugInDriverRef d, AudioObjectID obj, pid_t pid,
                                const AudioObjectPropertyAddress *a, UInt32 qSize, const void *q,
                                UInt32 size, const void *data) {
    (void)d; (void)pid; (void)qSize; (void)q;
    if (obj == kObjectID_Device && a->mSelector == kAudioDevicePropertyNominalSampleRate) {
        RETURN_IF(size != sizeof(Float64), kAudioHardwareBadPropertySizeError);
        return *(const Float64 *)data == HUSH_MIC_RATE ? 0 : kAudioHardwareIllegalOperationError;
    }
    if (obj == kObjectID_Stream && (a->mSelector == kAudioStreamPropertyVirtualFormat ||
                                    a->mSelector == kAudioStreamPropertyPhysicalFormat)) {
        RETURN_IF(size != sizeof(AudioStreamBasicDescription), kAudioHardwareBadPropertySizeError);
        AudioStreamBasicDescription want = Format(), got = *(const AudioStreamBasicDescription *)data;
        return got.mSampleRate == want.mSampleRate && got.mFormatID == want.mFormatID &&
               got.mChannelsPerFrame == want.mChannelsPerFrame && got.mBitsPerChannel == want.mBitsPerChannel
                   ? 0 : kAudioDeviceUnsupportedFormatError;
    }
    if (obj == kObjectID_Stream && a->mSelector == kAudioStreamPropertyIsActive) return 0;
    return kAudioHardwareUnknownPropertyError;
}

// ---------------------------------------------------------------------------------------------
// IO

static OSStatus StartIO(AudioServerPlugInDriverRef d, AudioObjectID dev, UInt32 client) {
    (void)d; (void)client;
    RETURN_IF(dev != kObjectID_Device, kAudioHardwareBadObjectError);
    pthread_mutex_lock(&gStateMutex);
    if (gIOCount++ == 0) {
        OpenShared(); // Retried at each start, outside the IO path.
        gTimeStampCount = 0;
        gAnchorHostTime = mach_absolute_time();
        gReader.synced = 0;
        os_log(gLog, "start: shared memory %{public}s", gShared ? "mapped" : "missing");
    }
    pthread_mutex_unlock(&gStateMutex);
    return 0;
}

static OSStatus StopIO(AudioServerPlugInDriverRef d, AudioObjectID dev, UInt32 client) {
    (void)d; (void)client;
    RETURN_IF(dev != kObjectID_Device, kAudioHardwareBadObjectError);
    pthread_mutex_lock(&gStateMutex);
    if (gIOCount > 0) gIOCount--;
    if (gIOCount == 0) {
        os_log(gLog, "stop: reads %llu, resyncs %llu, missing frames %llu, time jumps %llu",
               gReads, gReader.resyncs, gReader.missing, gTimeJumps);
        gReads = gTimeJumps = gReader.resyncs = gReader.missing = 0;
        gLastTime = -1;
    }
    pthread_mutex_unlock(&gStateMutex);
    return 0;
}

static OSStatus GetZeroTimeStamp(AudioServerPlugInDriverRef d, AudioObjectID dev, UInt32 client,
                                 Float64 *sampleTime, UInt64 *hostTime, UInt64 *seed) {
    (void)d; (void)client;
    RETURN_IF(dev != kObjectID_Device, kAudioHardwareBadObjectError);
    // A free-running 48 kHz clock on the host clock, as in NullAudio. Called on the IO thread.
    UInt64 now = mach_absolute_time();
    Float64 ticksPerPeriod = gHostTicksPerFrame * kZeroTimeStampPeriod;
    if (gAnchorHostTime + (UInt64)((gTimeStampCount + 1) * ticksPerPeriod) <= now) gTimeStampCount++;
    *sampleTime = (Float64)(gTimeStampCount * kZeroTimeStampPeriod);
    *hostTime = gAnchorHostTime + (UInt64)(gTimeStampCount * ticksPerPeriod);
    *seed = 1;
    return 0;
}

static OSStatus WillDoIOOperation(AudioServerPlugInDriverRef d, AudioObjectID dev, UInt32 client,
                                  UInt32 op, Boolean *willDo, Boolean *inPlace) {
    (void)d; (void)client;
    RETURN_IF(dev != kObjectID_Device, kAudioHardwareBadObjectError);
    *willDo = op == kAudioServerPlugInIOOperationReadInput;
    *inPlace = true;
    return 0;
}

static OSStatus BeginIOOperation(AudioServerPlugInDriverRef d, AudioObjectID dev, UInt32 client, UInt32 op,
                                 UInt32 frames, const AudioServerPlugInIOCycleInfo *cycle) {
    (void)d; (void)client; (void)op; (void)frames; (void)cycle;
    return dev == kObjectID_Device ? 0 : kAudioHardwareBadObjectError;
}

static OSStatus DoIOOperation(AudioServerPlugInDriverRef d, AudioObjectID dev, AudioObjectID stream,
                              UInt32 client, UInt32 op, UInt32 frames, const AudioServerPlugInIOCycleInfo *cycle,
                              void *main, void *secondary) {
    (void)d; (void)client; (void)cycle; (void)secondary;
    RETURN_IF(dev != kObjectID_Device || stream != kObjectID_Stream, kAudioHardwareBadObjectError);
    if (op == kAudioServerPlugInIOOperationReadInput && main) ReadShared(main, frames, cycle->mInputTime.mSampleTime);
    return 0;
}

static OSStatus EndIOOperation(AudioServerPlugInDriverRef d, AudioObjectID dev, UInt32 client, UInt32 op,
                               UInt32 frames, const AudioServerPlugInIOCycleInfo *cycle) {
    (void)d; (void)client; (void)op; (void)frames; (void)cycle;
    return dev == kObjectID_Device ? 0 : kAudioHardwareBadObjectError;
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
    return dev == kObjectID_Device ? 0 : kAudioHardwareBadObjectError;
}
static OSStatus RemoveDeviceClient(AudioServerPlugInDriverRef d, AudioObjectID dev,
                                   const AudioServerPlugInClientInfo *client) {
    (void)d; (void)client;
    return dev == kObjectID_Device ? 0 : kAudioHardwareBadObjectError;
}
static OSStatus PerformDeviceConfigurationChange(AudioServerPlugInDriverRef d, AudioObjectID dev,
                                                 UInt64 action, void *info) {
    (void)d; (void)action; (void)info;
    return dev == kObjectID_Device ? 0 : kAudioHardwareBadObjectError;
}
static OSStatus AbortDeviceConfigurationChange(AudioServerPlugInDriverRef d, AudioObjectID dev,
                                               UInt64 action, void *info) {
    (void)d; (void)action; (void)info;
    return dev == kObjectID_Device ? 0 : kAudioHardwareBadObjectError;
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
