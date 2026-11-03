//! The app side of the "Hush AI Speaker" virtual output device (macOS).
//!
//! Meeting apps (Zoom, Meet in a browser, Teams) choose "Hush AI Speaker" as their speaker. The
//! Core Audio plug-in (`virtual_mic/macos/`) mixes each IO cycle to mono and appends it to a
//! shared-memory ring (`HushSpeakerShared` in `virtual_mic/shared.h`). The plug-in is the only
//! writer and never waits: when nobody reads, it overwrites the oldest audio. This side keeps its
//! own read position, so reading never blocks and several readers do not disturb each other.
//!
//! Other systems return an error from [`VirtualSpeaker::open`].

use eyre::Result;

/// Sample rate of the virtual speaker.
pub const RATE: u32 = 48_000;
const CAPACITY: u64 = 16_384;
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
const MAGIC: u32 = 0x4853_504B;
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
const VERSION: u32 = 1;
/// The plug-in can overwrite up to one IO cycle (at most this many frames) beyond its published
/// position while the reader copies. Positions that close to being overwritten count as lost.
const WRITE_MARGIN: u64 = 4_096;
/// The largest latency bound [`VirtualSpeaker::read`] accepts: 12288 frames, 256 ms.
pub const MAX_LATENCY: usize = (CAPACITY - WRITE_MARGIN) as usize;
/// A write this recent means an app is playing to the device.
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
const ACTIVE_NS: u64 = 500_000_000;

/// Mirror of `HushSpeakerShared` in `virtual_mic/shared.h`.
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
#[repr(C, align(64))]
struct Shared {
    magic: u32,
    version: u32,
    sample_rate: u32,
    capacity: u32,
    _pad0: [u8; 48],
    write_pos: std::sync::atomic::AtomicU64,
    /// `mach_absolute_time()` of the last write, 0 before any write.
    write_host_time: std::sync::atomic::AtomicU64,
    write_frames: std::sync::atomic::AtomicU32,
    _pad1: [u8; 44],
    /// The app's read position (diagnostics only).
    read_pos: std::sync::atomic::AtomicU64,
    _pad2: [u8; 56],
    samples: [f32; CAPACITY as usize],
}

/// A reader of the installed Hush AI Speaker device.
pub struct VirtualSpeaker {
    ring: platform::Ring,
    read: u64,
    skipped: u64,
}

impl VirtualSpeaker {
    /// Open the installed plug-in's ring. Errors when the plug-in is missing or on other OSes.
    /// Reading starts at the newest audio: audio played before `open` is never returned.
    pub fn open() -> Result<Self> {
        let ring = platform::Ring::open()?;
        let read = ring.write_pos();
        Ok(Self {
            ring,
            read,
            skipped: 0,
        })
    }

    /// Copy up to out.len() newest-unread mono frames; returns how many. Never blocks.
    /// If more than `max_latency` frames are buffered it skips ahead to keep latency bounded;
    /// if the writer lapped the reader it resynchronises.
    ///
    /// `max_latency` is at most [`MAX_LATENCY`] (larger values are reduced to it). Frames are
    /// mono at [`RATE`], finite and in [-1, 1].
    pub fn read(&mut self, out: &mut [f32], max_latency: usize) -> usize {
        let count = read_ring(
            &self.ring,
            &mut self.read,
            &mut self.skipped,
            out,
            max_latency,
        );
        self.ring.publish_read(self.read);
        count
    }

    /// Frames buffered and unread right now.
    pub fn buffered(&self) -> usize {
        self.ring
            .write_pos()
            .saturating_sub(self.read)
            .min(CAPACITY) as usize
    }

    /// True when an app wrote to the device in the last ~500 ms (a call is playing).
    pub fn active(&self) -> bool {
        self.ring.active()
    }

    /// Frames dropped so far to keep latency bounded or after the writer lapped the reader.
    pub fn skipped(&self) -> u64 {
        self.skipped
    }
}

/// The writer's side of a ring, as the reader sees it.
trait Source {
    /// Frames written so far (acquire order: the samples before it are visible).
    fn write_pos(&self) -> u64;
    /// The sample at ring position `pos`. It may be overwritten while it is read.
    fn sample(&self, pos: u64) -> f32;
}

/// The read rule, separate from the shared memory so that it is tested on every system.
fn read_ring(
    source: &impl Source,
    read: &mut u64,
    skipped: &mut u64,
    out: &mut [f32],
    max_latency: usize,
) -> usize {
    let max_latency = (max_latency as u64).min(MAX_LATENCY as u64);
    // A second pass only happens when the writer overwrote what the first pass copied.
    for _ in 0..2 {
        let write = source.write_pos();
        if *read > write {
            // The plug-in recreated the ring (a new version or reinstall): start again.
            *read = write.saturating_sub(max_latency);
        } else if write - *read > max_latency {
            // Too far behind, or lapped: keep the newest `max_latency` frames.
            *skipped += write - max_latency - *read;
            *read = write - max_latency;
        }
        let count = (write - *read).min(out.len() as u64) as usize;
        for (i, slot) in out[..count].iter_mut().enumerate() {
            *slot = source.sample(*read + i as u64);
        }
        // Valid only if the writer has not reached these positions again since `write`.
        let oldest_valid = (source.write_pos() + WRITE_MARGIN).saturating_sub(CAPACITY);
        if *read >= oldest_valid {
            *read += count as u64;
            return count;
        }
    }
    0
}

/// Whether a write at host time `written` is within [`ACTIVE_NS`] of host time `now`.
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
fn recent(written: u64, now: u64, numer: u32, denom: u32) -> bool {
    if written == 0 || denom == 0 {
        return false;
    }
    let ago_ns = u128::from(now.saturating_sub(written)) * u128::from(numer) / u128::from(denom);
    ago_ns < u128::from(ACTIVE_NS)
}

#[cfg(target_os = "macos")]
mod platform {
    use super::{recent, Shared, Source, CAPACITY, MAGIC, RATE, VERSION};
    use std::ptr::NonNull;
    use std::sync::atomic::Ordering;

    #[repr(C)]
    #[derive(Default)]
    struct Timebase {
        numer: u32,
        denom: u32,
    }

    extern "C" {
        fn mach_absolute_time() -> u64;
        fn mach_timebase_info(info: *mut Timebase) -> i32;
    }

    pub struct Ring {
        shared: NonNull<Shared>,
        timebase: Timebase,
    }

    // SAFETY: the mapping lives until drop; shared fields are atomics, or samples that only the
    // plug-in writes.
    unsafe impl Send for Ring {}

    impl Ring {
        pub fn open() -> eyre::Result<Self> {
            let mut timebase = Timebase::default();
            // SAFETY: a valid out pointer.
            unsafe { mach_timebase_info(&mut timebase) };
            let ring = Self {
                shared: map()?,
                timebase,
            };
            let header = ring.shared();
            eyre::ensure!(
                header.magic == MAGIC
                    && header.version == VERSION
                    && header.sample_rate == RATE
                    && u64::from(header.capacity) == CAPACITY,
                "Hush AI Speaker driver version does not match this app"
            );
            Ok(ring)
        }

        fn shared(&self) -> &Shared {
            // SAFETY: valid for the lifetime of `self`.
            unsafe { self.shared.as_ref() }
        }

        pub fn publish_read(&self, read: u64) {
            self.shared().read_pos.store(read, Ordering::Relaxed);
        }

        pub fn active(&self) -> bool {
            let written = self.shared().write_host_time.load(Ordering::Relaxed);
            // SAFETY: no arguments; reads the host clock.
            let now = unsafe { mach_absolute_time() };
            recent(written, now, self.timebase.numer, self.timebase.denom)
        }
    }

    impl Source for Ring {
        fn write_pos(&self) -> u64 {
            self.shared().write_pos.load(Ordering::Acquire)
        }

        fn sample(&self, pos: u64) -> f32 {
            // SAFETY: a field address inside the live mapping; the index is below CAPACITY. The
            // plug-in may store this sample concurrently, so read it once, volatile.
            unsafe {
                let samples: *const f32 =
                    std::ptr::addr_of!((*self.shared.as_ptr()).samples).cast();
                samples.add((pos % CAPACITY) as usize).read_volatile()
            }
        }
    }

    impl Drop for Ring {
        fn drop(&mut self) {
            // SAFETY: the mapping came from `map` and is unmapped once.
            unsafe { libc::munmap(self.shared.as_ptr().cast(), std::mem::size_of::<Shared>()) };
        }
    }

    fn map() -> eyre::Result<NonNull<Shared>> {
        let name = c"/hush.speaker.v1";
        let size = std::mem::size_of::<Shared>();
        // SAFETY: plain POSIX calls with valid arguments; the descriptor is closed after mmap.
        unsafe {
            let fd = libc::shm_open(name.as_ptr(), libc::O_RDWR);
            if fd < 0 {
                eyre::bail!(
                    "Hush AI Speaker is not installed ({})",
                    std::io::Error::last_os_error()
                );
            }
            let mut stat: libc::stat = std::mem::zeroed();
            let sized = libc::fstat(fd, &mut stat) == 0 && stat.st_size as usize >= size;
            let map = if sized {
                libc::mmap(
                    std::ptr::null_mut(),
                    size,
                    libc::PROT_READ | libc::PROT_WRITE,
                    libc::MAP_SHARED,
                    fd,
                    0,
                )
            } else {
                libc::MAP_FAILED
            };
            libc::close(fd);
            if map == libc::MAP_FAILED {
                eyre::bail!("Cannot map the Hush AI Speaker buffer");
            }
            Ok(NonNull::new_unchecked(map.cast()))
        }
    }
}

#[cfg(not(target_os = "macos"))]
mod platform {
    use super::Source;

    pub struct Ring;

    impl Ring {
        pub fn open() -> eyre::Result<Self> {
            Err(eyre::eyre!(
                "Hush AI Speaker is not available on this system yet"
            ))
        }
        pub fn publish_read(&self, _: u64) {}
        pub fn active(&self) -> bool {
            false
        }
    }

    impl Source for Ring {
        fn write_pos(&self) -> u64 {
            0
        }
        fn sample(&self, _: u64) -> f32 {
            0.0
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::{Cell, RefCell};

    /// A ring in memory. Each sample encodes its position, so stale or skipped audio shows.
    struct Fake {
        write: Cell<u64>,
        ring: RefCell<Vec<f32>>,
        loads: Cell<u32>,
        /// Frames the writer adds just before the load with this number (load 2: while the
        /// reader copies).
        burst_at: Cell<(u32, u64)>,
    }

    fn value(pos: u64) -> f32 {
        (pos % 65_536) as f32
    }

    impl Fake {
        fn new(start: u64) -> Self {
            let fake = Self {
                write: Cell::new(start),
                ring: RefCell::new(vec![f32::NAN; CAPACITY as usize]),
                loads: Cell::new(0),
                burst_at: Cell::new((0, 0)),
            };
            // The ring holds the last CAPACITY positions before `start`.
            fake.write.set(start.saturating_sub(CAPACITY));
            fake.write_frames(start.min(CAPACITY));
            fake
        }

        fn write_frames(&self, count: u64) {
            let w = self.write.get();
            let mut ring = self.ring.borrow_mut();
            for pos in w..w + count {
                ring[(pos % CAPACITY) as usize] = value(pos);
            }
            self.write.set(w + count);
        }
    }

    impl Source for Fake {
        fn write_pos(&self) -> u64 {
            self.loads.set(self.loads.get() + 1);
            let (at, frames) = self.burst_at.get();
            if at == self.loads.get() {
                self.write_frames(frames);
            }
            self.write.get()
        }
        fn sample(&self, pos: u64) -> f32 {
            self.ring.borrow()[(pos % CAPACITY) as usize]
        }
    }

    struct Reader {
        read: u64,
        skipped: u64,
    }

    impl Reader {
        fn at(fake: &Fake) -> Self {
            Self {
                read: fake.write.get(),
                skipped: 0,
            }
        }
        fn read(&mut self, fake: &Fake, out: &mut [f32], max_latency: usize) -> usize {
            read_ring(fake, &mut self.read, &mut self.skipped, out, max_latency)
        }
    }

    /// `out` holds consecutive positions ending just before `end`.
    fn assert_run(out: &[f32], end: u64) {
        let start = end - out.len() as u64;
        for (i, &v) in out.iter().enumerate() {
            assert_eq!(
                v,
                value(start + i as u64),
                "frame {i} of a run ending at {end}"
            );
        }
    }

    #[test]
    fn layout_matches_shared_header() {
        use std::mem::{align_of, offset_of, size_of};
        assert_eq!(offset_of!(Shared, write_pos), 64);
        assert_eq!(offset_of!(Shared, write_host_time), 72);
        assert_eq!(offset_of!(Shared, write_frames), 80);
        assert_eq!(offset_of!(Shared, read_pos), 128);
        assert_eq!(offset_of!(Shared, samples), 192);
        assert_eq!(align_of::<Shared>(), 64);
        assert_eq!(size_of::<Shared>(), 192 + CAPACITY as usize * 4);
        assert!(CAPACITY.is_power_of_two());
    }

    #[test]
    fn starts_at_the_newest_audio() {
        let fake = Fake::new(100_000);
        let mut reader = Reader::at(&fake);
        let mut out = [0.0; 1024];
        assert_eq!(reader.read(&fake, &mut out, 4800), 0);
        fake.write_frames(480);
        assert_eq!(reader.read(&fake, &mut out, 4800), 480);
        assert_run(&out[..480], 100_480);
        assert_eq!(reader.read(&fake, &mut out, 4800), 0);
        assert_eq!(reader.skipped, 0);
    }

    #[test]
    fn partial_reads_continue_without_gaps() {
        let fake = Fake::new(0);
        let mut reader = Reader::at(&fake);
        fake.write_frames(2000);
        let mut out = [0.0; 512];
        let mut end = 0;
        for expected in [512, 512, 512, 464, 0] {
            let n = reader.read(&fake, &mut out, 4800);
            assert_eq!(n, expected);
            end += n as u64;
            assert_run(&out[..n], end);
        }
        assert_eq!(reader.skipped, 0);
    }

    #[test]
    fn skips_ahead_to_bound_latency() {
        let fake = Fake::new(50_000);
        let mut reader = Reader::at(&fake);
        fake.write_frames(10_000);
        let mut out = [0.0; 1000];
        assert_eq!(reader.read(&fake, &mut out, 4800), 1000);
        // The newest 4800 frames were kept; the first 1000 of them were read.
        assert_run(&out, 60_000 - 3800);
        assert_eq!(reader.skipped, 10_000 - 4800);
        assert_eq!(fake.write.get() - reader.read, 3800);
    }

    #[test]
    fn resynchronises_after_being_lapped() {
        let fake = Fake::new(1 << 40);
        let mut reader = Reader::at(&fake);
        for _ in 0..3 {
            fake.write_frames(CAPACITY - 1);
        }
        let mut out = [0.0; 256];
        assert_eq!(reader.read(&fake, &mut out, 2400), 256);
        assert_run(&out, (1 << 40) + 3 * (CAPACITY - 1) - 2400 + 256);
        assert!(out.iter().all(|v| v.is_finite()));
    }

    #[test]
    fn a_large_latency_bound_is_reduced() {
        let fake = Fake::new(0);
        let mut reader = Reader::at(&fake);
        fake.write_frames(CAPACITY * 2);
        let mut out = vec![0.0; CAPACITY as usize];
        assert_eq!(reader.read(&fake, &mut out, usize::MAX), MAX_LATENCY);
        assert_run(&out[..MAX_LATENCY], CAPACITY * 2);
    }

    #[test]
    fn discards_audio_overwritten_while_copying() {
        let fake = Fake::new(10_000);
        let mut reader = Reader::at(&fake);
        fake.write_frames(4000);
        // The writer adds a whole ring while the first pass copies (between loads 1 and 2).
        fake.loads.set(0);
        fake.burst_at.set((2, CAPACITY));
        let mut out = [0.0; 4096];
        let n = reader.read(&fake, &mut out, 4800);
        assert_eq!(n, 4096);
        let end = 14_000 + CAPACITY - 4800 + 4096;
        assert_run(&out[..n], end);
        assert_eq!(reader.read, end);
    }

    #[test]
    fn writer_restart_starts_again() {
        let fake = Fake::new(1_000_000);
        let mut reader = Reader::at(&fake);
        // The plug-in recreated the ring: positions start from 0 again.
        let fresh = Fake::new(0);
        fresh.write_frames(300);
        let mut out = [0.0; 1024];
        assert_eq!(reader.read(&fresh, &mut out, 4800), 300);
        assert_run(&out[..300], 300);
        drop(fake);
    }

    #[test]
    fn active_means_written_in_the_last_500_ms() {
        // Apple silicon: 125/3 ns per tick; Intel: 1/1.
        for (numer, denom) in [(125, 3), (1, 1)] {
            let ticks = |ms: u64| ms * 1_000_000 * u64::from(denom) / u64::from(numer);
            let now = ticks(10_000);
            assert!(!recent(0, now, numer, denom), "never written");
            assert!(recent(now, now, numer, denom));
            assert!(recent(now - ticks(400), now, numer, denom));
            assert!(!recent(now - ticks(600), now, numer, denom));
            assert!(
                recent(now + 5, now, numer, denom),
                "clock read before the write"
            );
        }
    }
}
