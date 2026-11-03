/* Edge-case tests for the shared reader in ring.h. Portable C; no driver needed.
 *   Windows: cl /O2 /W4 /WX virtual_mic\test_ring.c /Fe:target\test_ring.exe
 *   macOS:   clang -O2 -Wall -Wextra -Werror -o target/test_ring virtual_mic/test_ring.c
 * A simulated driver reads on the device clock; a simulated app writes with the same pacing
 * rule as hush_core::virtual_mic::Feeder (one read plus 10 ms ahead), with timing jitter.
 * Each sample encodes its ring position, so the test can see skips, repeats and stale audio.
 */
#include <math.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include "ring.h"

#define CAP 16384u
#define RATE 48 /* frames per millisecond */

static int failures;
#define CHECK(cond, ...) do { if (!(cond)) { failures++; printf("FAIL line %d: ", __LINE__); printf(__VA_ARGS__); printf("\n"); } } while (0)

typedef struct {
    float ring[CAP];
    int64_t write;
    int64_t read; /* furthest end published by readers */
    uint32_t read_frames;
    uint64_t rng;
} World;

/* Position p is stored as (p mod 1e6) * 1e-6, then decoded back. 0 is reserved for silence. */
static float encode(int64_t p) { return (float)((p % 1000000) + 1) * 1e-6f; }
static int64_t decode(float v) { return v == 0.0f ? -1 : (int64_t)llround(v * 1e6) - 1; }

static uint32_t rnd(World *w, uint32_t n) {
    w->rng = w->rng * 6364136223846793005ull + 1442695040888963407ull;
    return (uint32_t)(w->rng >> 33) % n;
}

static void app_write(World *w, int64_t frames) {
    for (int64_t i = 0; i < frames; i++) w->ring[(w->write + i) & (CAP - 1)] = encode(w->write + i);
    w->write += frames;
}

/* The feeder rule: keep one driver read plus 10 ms buffered beyond the furthest read. */
static void app_pass(World *w) {
    int64_t target = (int64_t)(w->read_frames < 64 ? 64 : w->read_frames > 4096 ? 4096 : w->read_frames) + 10 * RATE;
    /* The reader is past the newest sample: skip ahead to it, then fill as usual. */
    if (w->write < w->read) app_write(w, w->read - w->write);
    int64_t buffered = w->write - w->read;
    if (target > buffered) app_write(w, target - buffered);
}

static int64_t driver_read(World *w, HushRingReader *r, int64_t time, float *out, uint32_t count) {
    int64_t end = hush_ring_read(r, w->ring, CAP, w->write, time, out, count);
    if (end > w->read || w->read - end > (int64_t)CAP) {
        w->read = end;
        w->read_frames = count;
    }
    return end;
}

typedef struct {
    int64_t samples, audio, silent, skips, repeats, last;
} Stream;

static void track(Stream *s, const float *out, uint32_t n) {
    for (uint32_t i = 0; i < n; i++) {
        int64_t p = decode(out[i]);
        s->samples++;
        if (p < 0) { s->silent++; continue; }
        s->audio++;
        if (s->last >= 0) {
            int64_t step = (p - s->last + 1000000) % 1000000;
            if (step == 0 || step > 500000) s->repeats++;
            else if (step != 1) s->skips++;
        }
        s->last = p;
    }
}

/* Run the driver for `ms` milliseconds with reads of `count` frames. The app runs every
   `period` ms plus up to `jitter` ms (0 = never). Returns the stream statistics. */
static Stream run(World *w, HushRingReader *r, int64_t *time, uint32_t count, int ms, int period, int jitter) {
    Stream s = {0, 0, 0, 0, 0, -1};
    float out[4096];
    int64_t next_app = 0;
    int64_t now_frames = *time;
    int64_t end = *time + (int64_t)ms * RATE;
    int64_t next_read = *time + count; /* a read of [t, t+count) happens at t+count */
    while (next_read <= end) {
        int64_t next_app_frames = period ? *time + next_app : INT64_MAX;
        if (period && next_app_frames <= next_read) {
            now_frames = next_app_frames;
            app_pass(w);
            next_app += (int64_t)(period + (jitter ? (int)rnd(w, (uint32_t)jitter + 1) : 0)) * RATE;
            continue;
        }
        now_frames = next_read;
        driver_read(w, r, next_read - count, out, count);
        track(&s, out, count);
        next_read += count;
    }
    (void)now_frames;
    *time = end - (end - *time) % count;
    return s;
}

int main(void) {
    static World w;
    float out[4096], again[4096];
    uint32_t sizes[] = {1, 7, 48, 64, 441, 480, 512, 1024, 4096};

    /* 1. Before any write: silence, nothing counted as audio. */
    {
        memset(&w, 0, sizeof w);
        HushRingReader r = {0};
        int64_t t = 0;
        Stream s = run(&w, &r, &t, 512, 1000, 0, 0);
        CHECK(s.audio == 0, "silence before any write: %lld audio samples", (long long)s.audio);
    }
    /* 2. Old audio in the ring and no writer: never played. */
    {
        memset(&w, 0, sizeof w);
        app_write(&w, 12345);
        HushRingReader r = {0};
        int64_t t = 777;
        Stream s = run(&w, &r, &t, 512, 2000, 0, 0);
        CHECK(s.audio == 0, "stale ring with idle writer: %lld audio samples", (long long)s.audio);
    }
    /* 3. Writer jitter below the 10 ms margin, every read size: continuous audio. */
    for (unsigned k = 0; k < sizeof sizes / sizeof sizes[0]; k++) {
        memset(&w, 0, sizeof w);
        w.rng = k + 1;
        HushRingReader r = {0};
        int64_t t = 1000 * (int64_t)k;
        run(&w, &r, &t, sizes[k], 300, 2, 0); /* start up */
        Stream s = run(&w, &r, &t, sizes[k], 20000, 2, 6);
        CHECK(s.silent == 0 && s.skips == 0 && s.repeats == 0,
              "read %u frames, jitter 2-8 ms: silent %lld skips %lld repeats %lld", sizes[k],
              (long long)s.silent, (long long)s.skips, (long long)s.repeats);
    }
    /* 4. The app stops: buffered audio plays out, then silence. Nothing repeats. */
    {
        memset(&w, 0, sizeof w);
        HushRingReader r = {0};
        int64_t t = 0;
        run(&w, &r, &t, 512, 1000, 2, 0);
        Stream s = run(&w, &r, &t, 512, 5000, 0, 0);
        CHECK(s.repeats == 0 && s.skips == 0, "after stop: repeats %lld skips %lld", (long long)s.repeats, (long long)s.skips);
        CHECK(s.audio <= 512 + 10 * RATE + 4096, "after stop: %lld samples of old audio", (long long)s.audio);
        /* 5. The app starts again: audio within one read after its first write. */
        Stream back = run(&w, &r, &t, 512, 3000, 2, 0);
        CHECK(back.silent <= 2 * 512 && back.skips == 0 && back.repeats == 0,
              "restart: silent %lld skips %lld repeats %lld", (long long)back.silent, (long long)back.skips, (long long)back.repeats);
    }
    /* 6. The app starts after the driver: audio within one read. */
    {
        memset(&w, 0, sizeof w);
        HushRingReader r = {0};
        int64_t t = 0;
        run(&w, &r, &t, 480, 1000, 0, 0);
        Stream s = run(&w, &r, &t, 480, 3000, 2, 0);
        CHECK(s.silent <= 2 * 480 && s.skips == 0 && s.repeats == 0, "late writer: silent %lld skips %lld",
              (long long)s.silent, (long long)s.skips);
    }
    /* 7. Two clients at the same device time get the same samples (one shared reader). */
    {
        memset(&w, 0, sizeof w);
        HushRingReader r = {0};
        int64_t t = 0;
        run(&w, &r, &t, 512, 500, 2, 0);
        app_pass(&w);
        driver_read(&w, &r, t, out, 512);
        driver_read(&w, &r, t, again, 512);
        CHECK(memcmp(out, again, sizeof(float) * 512) == 0, "two clients at the same time differ");
    }
    /* 8. Gaps longer than the margin: some silence, but never repeats or backward steps. */
    {
        memset(&w, 0, sizeof w);
        w.rng = 99;
        HushRingReader r = {0};
        int64_t t = 0;
        run(&w, &r, &t, 512, 300, 2, 0);
        Stream s = run(&w, &r, &t, 512, 20000, 2, 25);
        CHECK(s.repeats == 0, "long gaps: %lld repeats", (long long)s.repeats);
        CHECK(s.audio > s.samples / 2, "long gaps: only %lld of %lld samples had audio", (long long)s.audio, (long long)s.samples);
    }
    /* 9. Huge positions and many ring wraps. */
    {
        memset(&w, 0, sizeof w);
        w.write = w.read = (int64_t)1 << 40;
        HushRingReader r = {0};
        int64_t t = ((int64_t)1 << 41) + 3;
        run(&w, &r, &t, 512, 300, 2, 0);
        Stream s = run(&w, &r, &t, 512, 60000, 2, 4);
        CHECK(s.silent == 0 && s.skips == 0 && s.repeats == 0, "huge positions: silent %lld skips %lld",
              (long long)s.silent, (long long)s.skips);
    }
    /* 10. Writer bursts far ahead: the reader jumps to recent audio (bounded delay). */
    {
        memset(&w, 0, sizeof w);
        HushRingReader r = {0};
        int64_t t = 0;
        run(&w, &r, &t, 512, 300, 2, 0);
        app_write(&w, 12000);
        driver_read(&w, &r, t, out, 512);
        int64_t first = decode(out[0]);
        CHECK(w.write - first <= 512 + HUSH_RING_LAG, "burst: delay %lld frames", (long long)(w.write - first));
    }
    /* 11. Device time jumps (skipped IO cycles): no crash, audio continues. */
    {
        memset(&w, 0, sizeof w);
        HushRingReader r = {0};
        int64_t t = 0;
        run(&w, &r, &t, 512, 300, 2, 0);
        t += 2048;
        Stream s = run(&w, &r, &t, 512, 2000, 2, 0);
        CHECK(s.repeats == 0 && s.audio > 0, "time jump: repeats %lld audio %lld", (long long)s.repeats, (long long)s.audio);
    }
    /* 12. Invalid values: NaN, infinities, out of range. */
    {
        memset(&w, 0, sizeof w);
        HushRingReader r = {0};
        int64_t t = 0;
        run(&w, &r, &t, 4, 10, 1, 0);
        float bad[4] = {NAN, INFINITY, -INFINITY, 3.0f};
        int64_t base = w.write;
        for (int i = 0; i < 4; i++) w.ring[(base + i) & (CAP - 1)] = bad[i];
        w.write += 4;
        r.offset = base - (t + 0); /* read exactly these four */
        hush_ring_read(&r, w.ring, CAP, w.write, t, out, 4);
        CHECK(out[0] == 0.0f && out[1] == 1.0f && out[2] == -1.0f && out[3] == 1.0f,
              "sanitize: %g %g %g %g", out[0], out[1], out[2], out[3]);
    }
    if (failures) {
        printf("%d check(s) failed\n", failures);
        return 1;
    }
    printf("ring: all checks passed\n");
    return 0;
}
