//! Scoped Windows Audio scheduling. Every handle is reverted on its registering thread.
use std::{ffi::c_void, io, marker::PhantomData, rc::Rc, sync::mpsc, thread};

#[link(name = "avrt")]
extern "system" {
    fn AvSetMmThreadCharacteristicsW(name: *const u16, index: *mut u32) -> *mut c_void;
    fn AvSetMmThreadPriority(handle: *mut c_void, priority: i32) -> i32;
    fn AvRevertMmThreadCharacteristics(handle: *mut c_void) -> i32;
}

/// Must be dropped on the registering thread; Rc marker forbids Send and Sync.
pub(crate) struct Registration {
    handle: *mut c_void,
    _thread_bound: PhantomData<Rc<()>>,
}

impl Registration {
    pub fn enter() -> io::Result<Self> {
        Self::named("Audio")
    }

    fn named(name: &str) -> io::Result<Self> {
        let name: Vec<u16> = name.encode_utf16().chain(Some(0)).collect();
        let mut index = 0;
        // SAFETY: name is NUL-terminated and index is writable. The returned handle
        // is used only on this thread, and reverted by this thread-bound guard.
        unsafe {
            let handle = AvSetMmThreadCharacteristicsW(name.as_ptr(), &mut index);
            if handle.is_null() {
                return Err(io::Error::last_os_error());
            }
            let guard = Self {
                handle,
                _thread_bound: PhantomData,
            };
            if AvSetMmThreadPriority(handle, 0) == 0 {
                return Err(io::Error::last_os_error());
            }
            Ok(guard)
        }
    }

    fn revert(&mut self) -> io::Result<()> {
        if self.handle.is_null() {
            return Ok(());
        }
        // SAFETY: the live handle belongs to this thread; the guard cannot move threads.
        if unsafe { AvRevertMmThreadCharacteristics(self.handle) } == 0 {
            return Err(io::Error::last_os_error());
        }
        self.handle = std::ptr::null_mut();
        Ok(())
    }
}

impl Drop for Registration {
    fn drop(&mut self) {
        if let Err(error) = self.revert() {
            eprintln!("mmcss cleanup failed: {error}");
        }
    }
}

pub(crate) fn pool(threads: usize) -> eyre::Result<rayon::ThreadPool> {
    pool_with_registration(threads, Registration::enter)
}

fn pool_with_registration<G: 'static, F>(
    threads: usize,
    enter: F,
) -> eyre::Result<rayon::ThreadPool>
where
    F: Fn() -> io::Result<G> + Clone + Send + 'static,
{
    let (sender, receiver) = mpsc::channel();
    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(threads)
        .spawn_handler(move |worker| {
            let mut builder = thread::Builder::new();
            if let Some(name) = worker.name() {
                builder = builder.name(name.to_owned());
            }
            if let Some(size) = worker.stack_size() {
                builder = builder.stack_size(size);
            }
            let sender = sender.clone();
            let enter = enter.clone();
            builder.spawn(move || {
                let registration = enter();
                let status = registration
                    .as_ref()
                    .map(|_| ())
                    .map_err(ToString::to_string);
                let _ = sender.send(status);
                // Run even after registration fails so Rayon can finish construction
                // and safely tear down the rejected pool. Guard drops on this thread.
                worker.run();
                drop(registration);
            })?;
            Ok(())
        })
        .build()?;
    for _ in 0..pool.current_num_threads() {
        receiver.recv()?.map_err(|error| eyre::eyre!(error))?;
    }
    Ok(pool)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    struct Observed {
        registration: Registration,
        started: thread::ThreadId,
        ended: mpsc::Sender<(thread::ThreadId, thread::ThreadId, bool)>,
    }

    impl Drop for Observed {
        fn drop(&mut self) {
            let ok = self.registration.revert().is_ok();
            let _ = self.ended.send((self.started, thread::current().id(), ok));
        }
    }

    #[test]
    fn pool_reverts_every_successful_registration_even_after_startup_failure() {
        let Ok(probe) = Registration::enter() else {
            eprintln!("MMCSS unavailable on this host; lifecycle integration case skipped");
            return;
        };
        drop(probe);
        for fail_one in [false, true] {
            let (sender, receiver) = mpsc::channel();
            let counter = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
            let pool = pool_with_registration(4, move || {
                let index = counter.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                if fail_one && index == 1 {
                    return Err(io::Error::other("injected registration failure"));
                }
                Ok(Observed {
                    registration: Registration::enter()?,
                    started: thread::current().id(),
                    ended: sender.clone(),
                })
            });
            assert_eq!(pool.is_err(), fail_one);
            if let Ok(pool) = pool {
                assert_eq!(pool.install(|| 7 * 6), 42);
                drop(pool);
            }
            let mut ids = std::collections::HashSet::new();
            for _ in 0..if fail_one { 3 } else { 4 } {
                let (started, ended, reverted) =
                    receiver.recv_timeout(Duration::from_secs(10)).unwrap();
                assert_eq!(started, ended);
                assert!(reverted);
                assert!(ids.insert(ended));
            }
        }
        eprintln!("MMCSS lifecycle verified: successful pool and partial-startup failure");
    }

    #[test]
    fn registration_reverts_on_its_thread_and_can_be_repeated() {
        thread::spawn(|| {
            if Registration::enter().is_err() {
                eprintln!("MMCSS unavailable on this host; registration integration case skipped");
                return;
            }
            for _ in 0..3 {
                let mut guard = Registration::named("Audio").unwrap();
                guard.revert().unwrap();
                assert!(guard.handle.is_null());
            }
        })
        .join()
        .unwrap();
    }

    #[test]
    fn invalid_task_returns_error() {
        assert!(Registration::named("Hush nonexistent MMCSS task").is_err());
    }
}
