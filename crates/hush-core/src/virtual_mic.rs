//! The app side of the "Hush AI Microphone" virtual device.
//!
//! The driver (`virtual_mic/`) keeps a ring of recent audio and reads it in its audio
//! callback; this side writes enhanced audio into it. One writer per ring. Writing never
//! blocks. The driver maps its own sample time to ring positions, so every app that records
//! the microphone gets the same audio, and it skips ahead when the writer is far ahead or
//! behind.
//!
//! - macOS: a shared-memory ring that the Core Audio plug-in creates (`virtual_mic/shared.h`).
//! - Windows: a kernel ring behind the control device `\\.\HushAIMicrophone`
//!   (`virtual_mic/windows/hushmic.h`).
//!
//! Other systems return an error from [`VirtualMic::open`].

use eyre::Result;

/// Sample rate of the virtual microphone.
pub const RATE: u32 = 48_000;
const CAPACITY: u64 = 16_384;
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
const MAGIC: u32 = 0x4855_5348;
const VERSION: u32 = 1;

/// Mirror of `HushMicShared` in `virtual_mic/shared.h`.
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
#[repr(C, align(64))]
struct Shared {
    magic: u32,
    version: u32,
    sample_rate: u32,
    capacity: u32,
    /// Process ID of the writing app, 0 if none: one writer at a time.
    writer_pid: std::sync::atomic::AtomicU32,
    _pad0: [u8; 44],
    write_pos: std::sync::atomic::AtomicU64,
    _pad1: [u8; 56],
    read_pos: std::sync::atomic::AtomicU64,
    read_frames: std::sync::atomic::AtomicU32,
    _pad2: [u8; 52],
    samples: [f32; CAPACITY as usize],
}

/// A connection to the installed Hush AI Microphone driver.
pub struct VirtualMic {
    ring: platform::Ring,
}

/// Read state of the driver.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Position {
    /// Frames written and not yet read by the driver.
    pub buffered: u64,
    /// Frames the driver has read past the newest written frame (the writer fell behind).
    pub behind: u64,
    /// Frames the driver has read in total. It stops moving when no app uses the microphone.
    pub read: u64,
    /// Size of the driver's last read, in frames (0 before the first read).
    pub read_frames: u32,
}

impl VirtualMic {
    /// Connect to the installed driver and clear audio left from an earlier session.
    pub fn open() -> Result<Self> {
        Ok(Self {
            ring: platform::Ring::open()?,
        })
    }

    /// Watch the driver's read position without writing (macOS only; for tests). It does not
    /// take the writer slot and does not clear audio.
    pub fn open_observer() -> Result<Self> {
        Ok(Self {
            ring: platform::Ring::observe()?,
        })
    }

    pub fn position(&self) -> Position {
        self.ring.position()
    }

    /// Append mono samples at [`RATE`].
    pub fn write(&mut self, samples: &[f32]) {
        self.ring.write(samples);
    }
}

/// Fills the virtual microphone from a normal thread, paced by the driver's reads.
///
/// The driver reads the ring on its own clock. The feeder keeps one driver read plus 10 ms
/// buffered, so delay stays bounded and the source follows the driver's clock. When no app
/// reads the microphone, the feeder keeps pace with the system clock so the source does not
/// back up; the driver skips that old audio when a reader starts. Dropping the feeder stops it.
pub struct Feeder {
    done: std::sync::Arc<std::sync::atomic::AtomicBool>,
    /// Longest time between two passes of the feeder loop, in microseconds.
    max_gap: std::sync::Arc<std::sync::atomic::AtomicU64>,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl Feeder {
    /// `fill` must fill its whole slice with samples at [`RATE`]. It runs on the feeder thread.
    pub fn start(
        mut mic: VirtualMic,
        mut fill: impl FnMut(&mut [f32]) + Send + 'static,
    ) -> Result<Self> {
        use std::sync::atomic::{AtomicBool, Ordering};
        use std::time::{Duration, Instant};
        let done = std::sync::Arc::new(AtomicBool::new(false));
        let finish = done.clone();
        let max_gap = std::sync::Arc::new(std::sync::atomic::AtomicU64::new(0));
        let gap = max_gap.clone();
        let rate = u64::from(RATE);
        let thread = std::thread::Builder::new()
            .name("hush-virtual-mic".into())
            .spawn(move || {
                let _audio = crate::audio_thread::periodic(Duration::from_millis(2));
                let mut block = [0f32; 1024];
                let mut last_read = mic.position().read;
                let mut active_at = Instant::now();
                let mut pulled_at = Instant::now();
                let mut passed_at = Instant::now();
                while !finish.load(Ordering::Acquire) {
                    let now = Instant::now();
                    gap.fetch_max(
                        now.duration_since(passed_at).as_micros() as u64,
                        Ordering::Relaxed,
                    );
                    passed_at = now;
                    let position = mic.position();
                    if position.read != last_read {
                        last_read = position.read;
                        active_at = now;
                    }
                    let wanted = if now.duration_since(active_at) > Duration::from_millis(200) {
                        (now.duration_since(pulled_at).as_secs_f64() * rate as f64) as u64
                    } else {
                        let target = u64::from(position.read_frames.clamp(64, 4096)) + rate / 100;
                        target.saturating_sub(position.buffered)
                    };
                    pulled_at = now;
                    // The driver read past the newest sample: skip ahead with silence, so new
                    // audio lands where it reads next instead of being pulled too fast.
                    let mut skip = position.behind.min(CAPACITY) as usize;
                    while skip > 0 {
                        let count = skip.min(block.len());
                        block[..count].fill(0.0);
                        mic.write(&block[..count]);
                        skip -= count;
                    }
                    let mut left = wanted.min(rate / 10) as usize;
                    while left > 0 {
                        let count = left.min(block.len());
                        fill(&mut block[..count]);
                        mic.write(&block[..count]);
                        left -= count;
                    }
                    std::thread::sleep(Duration::from_millis(2));
                }
            })?;
        Ok(Self {
            done,
            max_gap,
            thread: Some(thread),
        })
    }

    /// Longest time between two passes of the feeder loop since start. The ring holds about
    /// 10 ms beyond one driver read, so gaps longer than that cause silence.
    pub fn max_gap(&self) -> std::time::Duration {
        std::time::Duration::from_micros(self.max_gap.load(std::sync::atomic::Ordering::Relaxed))
    }
}

impl Drop for Feeder {
    fn drop(&mut self) {
        self.done.store(true, std::sync::atomic::Ordering::Release);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

#[cfg(target_os = "macos")]
mod platform {
    use super::{Position, Shared, CAPACITY, MAGIC, RATE, VERSION};
    use std::ptr::NonNull;
    use std::sync::atomic::Ordering;

    pub struct Ring {
        shared: NonNull<Shared>,
        /// Holds the writer slot (false for an observer).
        writer: bool,
    }

    // SAFETY: the mapping lives until drop; shared fields are atomics or samples that only this
    // single writer changes.
    unsafe impl Send for Ring {}

    impl Ring {
        pub fn observe() -> eyre::Result<Self> {
            let ring = Self {
                shared: map()?,
                writer: false,
            };
            ring.check()?;
            Ok(ring)
        }

        fn check(&self) -> eyre::Result<()> {
            let header = self.shared();
            eyre::ensure!(
                header.magic == MAGIC
                    && header.version == VERSION
                    && header.sample_rate == RATE
                    && u64::from(header.capacity) == CAPACITY,
                "Hush AI Microphone driver version does not match this app"
            );
            Ok(())
        }

        pub fn open() -> eyre::Result<Self> {
            let mut ring = Self {
                shared: map()?,
                writer: false,
            };
            ring.check()?;
            // SAFETY: the mapping stays valid until `ring` drops.
            let header: &Shared = unsafe { ring.shared.as_ref() };
            // Take the writer slot. A slot held by a process that no longer exists is free.
            let me = std::process::id();
            let mut owner = header.writer_pid.load(Ordering::Acquire);
            loop {
                // SAFETY: signal 0 only checks that the process exists.
                let alive = owner != 0 && unsafe { libc::kill(owner as libc::pid_t, 0) } == 0;
                if alive && owner != me {
                    // `ring` is not the writer, so dropping it keeps the other writer's slot.
                    eyre::bail!("Hush AI Microphone is in use by another Hush window");
                }
                match header.writer_pid.compare_exchange(
                    owner,
                    me,
                    Ordering::AcqRel,
                    Ordering::Acquire,
                ) {
                    Ok(_) => break,
                    Err(current) => owner = current,
                }
            }
            ring.writer = true;
            let samples = ring.samples();
            for index in 0..CAPACITY as usize {
                // SAFETY: inside the mapped ring; the driver may read these samples concurrently.
                unsafe { samples.add(index).write_volatile(0.0) };
            }
            Ok(ring)
        }

        fn shared(&self) -> &Shared {
            // SAFETY: valid for the lifetime of `self`.
            unsafe { self.shared.as_ref() }
        }

        fn samples(&self) -> *mut f32 {
            // SAFETY: a field address inside the live mapping.
            unsafe { std::ptr::addr_of_mut!((*self.shared.as_ptr()).samples) }.cast()
        }

        pub fn position(&self) -> Position {
            let shared = self.shared();
            let write = shared.write_pos.load(Ordering::Relaxed);
            let read = shared.read_pos.load(Ordering::Acquire);
            Position {
                buffered: write.saturating_sub(read),
                behind: read.saturating_sub(write),
                read,
                read_frames: shared.read_frames.load(Ordering::Relaxed),
            }
        }

        pub fn write(&mut self, samples: &[f32]) {
            let write = self.shared().write_pos.load(Ordering::Relaxed);
            let ring = self.samples();
            for (i, &sample) in samples.iter().enumerate() {
                let index = (write + i as u64) % CAPACITY;
                // SAFETY: only this writer stores samples; the driver reads them after the
                // release below.
                unsafe { ring.add(index as usize).write_volatile(sample) };
            }
            self.shared()
                .write_pos
                .store(write + samples.len() as u64, Ordering::Release);
        }
    }

    impl Drop for Ring {
        fn drop(&mut self) {
            if self.writer {
                let _ = self.shared().writer_pid.compare_exchange(
                    std::process::id(),
                    0,
                    Ordering::AcqRel,
                    Ordering::Relaxed,
                );
            }
            // SAFETY: the mapping came from `map` and is unmapped once.
            unsafe { libc::munmap(self.shared.as_ptr().cast(), std::mem::size_of::<Shared>()) };
        }
    }

    fn map() -> eyre::Result<NonNull<Shared>> {
        let name = c"/hush.mic.v1";
        let size = std::mem::size_of::<Shared>();
        // SAFETY: plain POSIX calls with valid arguments; the descriptor is closed after mmap.
        unsafe {
            let fd = libc::shm_open(name.as_ptr(), libc::O_RDWR);
            if fd < 0 {
                eyre::bail!(
                    "Hush AI Microphone is not installed ({})",
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
                eyre::bail!("Cannot map the Hush AI Microphone buffer");
            }
            Ok(NonNull::new_unchecked(map.cast()))
        }
    }
}

#[cfg(target_os = "windows")]
mod platform {
    use super::{Position, CAPACITY, VERSION};
    use std::ffi::c_void;

    /// `CTL_CODE(0x8A75, 0x800, METHOD_BUFFERED, FILE_WRITE_ACCESS)` in `hushmic.h`.
    const IOCTL_HUSH_MIC_WRITE: u32 = 0x8A75_A000;
    const MAX_WRITE: usize = 4096;

    #[link(name = "kernel32")]
    extern "system" {
        fn CreateFileW(
            name: *const u16,
            access: u32,
            share: u32,
            security: *mut c_void,
            disposition: u32,
            flags: u32,
            template: *mut c_void,
        ) -> *mut c_void;
        fn DeviceIoControl(
            device: *mut c_void,
            code: u32,
            input: *const c_void,
            input_size: u32,
            output: *mut c_void,
            output_size: u32,
            returned: *mut u32,
            overlapped: *mut c_void,
        ) -> i32;
        fn CloseHandle(handle: *mut c_void) -> i32;
    }

    /// `HUSH_MIC_POSITION` in `hushmic.h`.
    #[repr(C)]
    #[derive(Default)]
    struct DriverPosition {
        write: u64,
        read: u64,
        read_frames: u32,
        version: u32,
    }

    pub struct Ring {
        handle: *mut c_void,
    }

    // SAFETY: the handle is closed once; writes take `&mut`, so one thread writes at a time.
    unsafe impl Send for Ring {}

    impl Ring {
        pub fn observe() -> eyre::Result<Self> {
            Err(eyre::eyre!(
                "The Windows driver allows one handle: the writer"
            ))
        }

        pub fn open() -> eyre::Result<Self> {
            let name: Vec<u16> = r"\\.\HushAIMicrophone"
                .encode_utf16()
                .chain(Some(0))
                .collect();
            const GENERIC_READ: u32 = 0x8000_0000;
            const GENERIC_WRITE: u32 = 0x4000_0000;
            const OPEN_EXISTING: u32 = 3;
            // SAFETY: NUL-terminated name; null optional pointers.
            let handle = unsafe {
                CreateFileW(
                    name.as_ptr(),
                    GENERIC_READ | GENERIC_WRITE,
                    0,
                    std::ptr::null_mut(),
                    OPEN_EXISTING,
                    0,
                    std::ptr::null_mut(),
                )
            };
            if handle as isize == -1 {
                let error = std::io::Error::last_os_error();
                eyre::bail!(match error.raw_os_error() {
                    Some(2) | Some(3) => "Hush AI Microphone is not installed".to_owned(),
                    Some(5) => "Hush AI Microphone is in use by another Hush window".to_owned(),
                    _ => format!("Cannot open the Hush AI Microphone: {error}"),
                });
            }
            let mut ring = Self { handle };
            let position = ring.call(&[])?;
            eyre::ensure!(
                position.version == VERSION,
                "Hush AI Microphone driver version does not match this app"
            );
            // Push silence through the ring, so a reader never hears an earlier session.
            ring.write(&[0.0; CAPACITY as usize]);
            Ok(ring)
        }

        fn call(&self, samples: &[f32]) -> eyre::Result<DriverPosition> {
            let mut position = DriverPosition::default();
            let mut returned = 0;
            // SAFETY: valid buffers with their sizes; synchronous call on our own handle.
            let ok = unsafe {
                DeviceIoControl(
                    self.handle,
                    IOCTL_HUSH_MIC_WRITE,
                    samples.as_ptr().cast(),
                    std::mem::size_of_val(samples) as u32,
                    std::ptr::addr_of_mut!(position).cast(),
                    std::mem::size_of::<DriverPosition>() as u32,
                    &mut returned,
                    std::ptr::null_mut(),
                )
            };
            eyre::ensure!(
                ok != 0 && returned as usize == std::mem::size_of::<DriverPosition>(),
                "Hush AI Microphone request failed: {}",
                std::io::Error::last_os_error()
            );
            Ok(position)
        }

        pub fn position(&self) -> Position {
            self.call(&[]).map_or(Position::default(), |p| Position {
                buffered: p.write.saturating_sub(p.read),
                behind: p.read.saturating_sub(p.write),
                read: p.read,
                read_frames: p.read_frames,
            })
        }

        pub fn write(&mut self, samples: &[f32]) {
            for chunk in samples.chunks(MAX_WRITE) {
                // A removed driver shows up as a stalled position; the feeder keeps going.
                let _ = self.call(chunk);
            }
        }
    }

    impl Drop for Ring {
        fn drop(&mut self) {
            // SAFETY: our own handle, closed once.
            unsafe { CloseHandle(self.handle) };
        }
    }
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
mod platform {
    use super::Position;

    pub struct Ring;

    impl Ring {
        pub fn open() -> eyre::Result<Self> {
            Err(eyre::eyre!(
                "Hush AI Microphone is not available on this system yet"
            ))
        }
        pub fn observe() -> eyre::Result<Self> {
            Self::open()
        }
        pub fn position(&self) -> Position {
            Position::default()
        }
        pub fn write(&mut self, _: &[f32]) {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layout_matches_shared_header() {
        use std::mem::{align_of, offset_of, size_of};
        assert_eq!(offset_of!(Shared, writer_pid), 16);
        assert_eq!(offset_of!(Shared, write_pos), 64);
        assert_eq!(offset_of!(Shared, read_pos), 128);
        assert_eq!(offset_of!(Shared, read_frames), 136);
        assert_eq!(offset_of!(Shared, samples), 192);
        assert_eq!(align_of::<Shared>(), 64);
        assert_eq!(size_of::<Shared>(), 192 + CAPACITY as usize * 4);
    }
}
