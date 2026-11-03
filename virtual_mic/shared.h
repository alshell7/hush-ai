// Shared-memory ring between the Hush app (writer) and a virtual microphone driver (reader).
// Single producer, single consumer. Positions only grow; the index is position % capacity.
// The driver creates the region; the app opens it. Neither side blocks or takes a lock.
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
