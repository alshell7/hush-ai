/* The Hush AI Microphone reader, shared by the macOS plug-in and the Windows driver.
 *
 * The app writes mono samples into a ring and advances a write position. A driver reads
 * device time [time, time + count): position = time + offset. Rules:
 * - Several readers (apps) at the same device time get the same samples.
 * - The reader stays HUSH_RING_LAG frames behind the newest sample after a resync.
 * - More than HUSH_RING_MAX_DRIFT frames off while the writer is active: resync to the
 *   newest audio.
 * - The writer is idle when its position has not moved for HUSH_RING_MAX_DRIFT reader frames,
 *   or since the reader started. Then the reader outputs silence: it never replays old audio.
 * - Samples are finite and in [-1, 1].
 * Pure C, no locks, no allocation, no system calls: safe in a real-time audio callback.
 * The caller loads `write` with acquire order and publishes the returned end position.
 */
#ifndef HUSH_RING_H
#define HUSH_RING_H

#ifdef _KERNEL_MODE
/* Kernel builds have no C runtime headers. */
typedef signed __int64 hush_i64;
typedef unsigned __int64 hush_u64;
typedef unsigned int hush_u32;
#else
#include <stdint.h>
typedef int64_t hush_i64;
typedef uint64_t hush_u64;
typedef uint32_t hush_u32;
#endif

#define HUSH_RING_LAG 480        /* 10 ms at 48 kHz */
#define HUSH_RING_MAX_DRIFT 4800 /* 100 ms at 48 kHz */

typedef struct {
    hush_i64 offset;
    hush_i64 last_write;
    hush_i64 idle_frames;
    int synced;
    /* Diagnostics: resyncs, and frames with no audio while the writer was active. */
    hush_u64 resyncs;
    hush_u64 missing;
} HushRingReader;

/* Set synced = 0 when a stream starts; the rest is set on the first read. */
static inline hush_i64 hush_ring_read(HushRingReader *r, const volatile float *ring, hush_u32 capacity,
                                     hush_i64 write, hush_i64 time, float *out, hush_u32 count) {
    if (!r->synced) {
        r->synced = 1;
        r->last_write = write;
        r->idle_frames = HUSH_RING_MAX_DRIFT + 1; /* Audio already in the ring is old. */
        r->offset = write - (hush_i64)count - HUSH_RING_LAG - time;
    }
    if (write != r->last_write) {
        r->last_write = write;
        r->idle_frames = 0;
    } else if (r->idle_frames <= HUSH_RING_MAX_DRIFT) {
        r->idle_frames += count;
    }
    hush_i64 start = time + r->offset;
    if (r->idle_frames > HUSH_RING_MAX_DRIFT) {
        for (hush_u32 i = 0; i < count; i++) out[i] = 0.0f;
        return start + (hush_i64)count;
    }
    if (write - start > HUSH_RING_MAX_DRIFT + (hush_i64)count || start - write > HUSH_RING_MAX_DRIFT) {
        r->offset = write - (hush_i64)count - HUSH_RING_LAG - time;
        start = time + r->offset;
        r->resyncs++;
    }
    for (hush_u32 i = 0; i < count; i++) {
        hush_i64 p = start + (hush_i64)i;
        float v = 0.0f;
        if (p >= 0 && p < write && write - p <= (hush_i64)capacity) {
            v = ring[(hush_u64)p & (capacity - 1)];
            v = v != v ? 0.0f : v > 1.0f ? 1.0f : v < -1.0f ? -1.0f : v;
        } else {
            r->missing++;
        }
        out[i] = v;
    }
    return start + (hush_i64)count;
}

#endif
