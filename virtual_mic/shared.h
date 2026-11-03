// Shared-memory rings between the Hush app and the Core Audio plug-in / virtual microphone driver.
// Single producer, single consumer. Positions only grow; the index is position % capacity.
// The driver creates each region; the app opens it. Neither side blocks or takes a lock.
//
// Hush AI Microphone: the app writes, the driver reads.
#pragma once
#include <stdint.h>

#define HUSH_MIC_MAGIC 0x48555348u /* "HUSH" */
#define HUSH_MIC_VERSION 1u
#define HUSH_MIC_RATE 48000u
#define HUSH_MIC_CAPACITY 16384u /* frames, power of two: 341 ms at 48 kHz */
#define HUSH_MIC_SHM_NAME "/hush.mic.v1"

typedef struct {
    uint32_t magic;
    uint32_t version;
    uint32_t sample_rate;
    uint32_t capacity;
    /* Process ID of the app that writes, 0 if none. Only the app uses it: one writer at a time. */
    uint32_t writer_pid;
    uint8_t pad0[44];
    /* Frames written by the app. The app stores it with release order after writing samples. */
    _Alignas(64) uint64_t write_pos;
    uint8_t pad1[56];
    /* Frames consumed by the driver, and the size of its last read (frames). */
    _Alignas(64) uint64_t read_pos;
    uint32_t read_frames;
    uint8_t pad2[52];
    /* Mono float32 samples at sample_rate. */
    _Alignas(64) float samples[HUSH_MIC_CAPACITY];
} HushMicShared;

/* Hush AI Speaker: the reverse direction. Meeting apps play into the virtual output device; the
 * plug-in mixes each IO cycle to mono and appends it here (plug-in = writer, app = reader).
 * The plug-in creates the region and never waits for a reader: it overwrites the oldest audio.
 * The app keeps its own read position and only checks that the writer has not lapped it. */
#define HUSH_SPEAKER_MAGIC 0x4853504Bu /* "HSPK" */
#define HUSH_SPEAKER_VERSION 1u
#define HUSH_SPEAKER_RATE 48000u
#define HUSH_SPEAKER_CHANNELS 2u      /* the device format; the ring holds the mono mix */
#define HUSH_SPEAKER_CAPACITY 16384u  /* frames, power of two: 341 ms at 48 kHz */
#define HUSH_SPEAKER_SHM_NAME "/hush.speaker.v1"

typedef struct {
    uint32_t magic;
    uint32_t version;
    uint32_t sample_rate;
    uint32_t capacity;
    uint8_t pad0[48];
    /* Frames written by the plug-in. Stored with release order after the samples, the host
     * time and the frame count. */
    _Alignas(64) uint64_t write_pos;
    /* mach_absolute_time() of the last write, and its size in frames. 0 before any write. */
    uint64_t write_host_time;
    uint32_t write_frames;
    uint8_t pad1[44];
    /* Frames consumed by the app (diagnostics only; the plug-in does not read it). */
    _Alignas(64) uint64_t read_pos;
    uint8_t pad2[56];
    /* Mono float32 samples at sample_rate: the mean of the two channels. */
    _Alignas(64) float samples[HUSH_SPEAKER_CAPACITY];
} HushSpeakerShared;
