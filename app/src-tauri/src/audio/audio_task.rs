//! Own blocking device work on a control thread. Dropping the UI handle only requests stop.
use eyre::{eyre, Result};
use std::{
    panic::{catch_unwind, AssertUnwindSafe},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    thread,
};

pub struct AudioTask {
    stop: Arc<AtomicBool>,
}

impl AudioTask {
    pub fn spawn<T: Send + 'static>(
        name: &str,
        work: impl FnOnce(Arc<AtomicBool>) -> Result<T> + Send + 'static,
        complete: impl FnOnce(Result<T>) + Send + 'static,
    ) -> Result<Self> {
        let stop = Arc::new(AtomicBool::new(false));
        let signal = stop.clone();
        thread::Builder::new().name(name.into()).spawn(move || {
            // Device handles unwind on this thread before the terminal notification.
            let result = catch_unwind(AssertUnwindSafe(|| work(signal))).unwrap_or_else(|_| {
                Err(eyre!(
                    "Audio task stopped unexpectedly. Restart Hush if it repeats."
                ))
            });
            complete(result);
        })?;
        Ok(Self { stop })
    }

    pub fn request_stop(&self) {
        self.stop.store(true, Ordering::Release);
    }

    #[cfg(test)]
    pub fn is_stopping(&self) -> bool {
        self.stop.load(Ordering::Acquire)
    }
}

impl Drop for AudioTask {
    fn drop(&mut self) {
        self.request_stop();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{sync::mpsc, time::Duration};

    #[test]
    fn stop_during_blocked_startup_is_nonblocking_and_finishes_after_device_drop() {
        let caller = thread::current().id();
        let (opened_tx, opened_rx) = mpsc::channel();
        let (release_tx, release_rx) = mpsc::channel();
        let (events_tx, events_rx) = mpsc::channel();
        struct Device(mpsc::Sender<&'static str>);
        impl Drop for Device {
            fn drop(&mut self) {
                self.0.send("closed").unwrap();
            }
        }
        let closed = events_tx.clone();
        let task = AudioTask::spawn(
            "test-startup",
            move |stop| {
                assert_ne!(caller, thread::current().id());
                let _device = Device(closed);
                opened_tx.send(()).unwrap();
                release_rx.recv_timeout(Duration::from_secs(5)).unwrap();
                assert!(stop.load(Ordering::Acquire));
                Ok(())
            },
            move |result| {
                result.unwrap();
                events_tx.send("finished").unwrap();
            },
        )
        .unwrap();
        opened_rx.recv_timeout(Duration::from_secs(5)).unwrap();
        task.request_stop();
        assert!(task.is_stopping());
        drop(task); // Must not wait for the simulated blocked driver.
        assert!(matches!(
            events_rx.try_recv(),
            Err(mpsc::TryRecvError::Empty)
        ));
        release_tx.send(()).unwrap();
        assert_eq!(
            events_rx.recv_timeout(Duration::from_secs(5)).unwrap(),
            "closed"
        );
        assert_eq!(
            events_rx.recv_timeout(Duration::from_secs(5)).unwrap(),
            "finished"
        );
    }

    #[test]
    fn panic_still_produces_one_terminal_error() {
        let (tx, rx) = mpsc::channel();
        let _task = AudioTask::spawn::<()>(
            "test-panic",
            |_| panic!("simulated worker failure"),
            move |result| tx.send(result.map_err(|e| e.to_string())).unwrap(),
        )
        .unwrap();
        let error = rx
            .recv_timeout(Duration::from_secs(5))
            .unwrap()
            .unwrap_err();
        assert!(error.contains("Audio task stopped unexpectedly"));
        assert!(matches!(
            rx.recv_timeout(Duration::from_secs(5)),
            Err(mpsc::RecvTimeoutError::Disconnected)
        ));
    }
}
