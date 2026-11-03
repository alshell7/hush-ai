//! Atomic stop causes shared with callbacks; messages are built on control/UI threads.
use hush_core::live::LiveStats;
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum StopReason {
    Requested = 1,
    InputUnavailable,
    InputError,
    OutputUnavailable,
    OutputError,
    WorkerFailed,
}

impl StopReason {
    pub fn message(self) -> &'static str {
        match self {
            Self::Requested => "Live monitoring stopped.",
            Self::InputUnavailable => "Microphone disconnected or unavailable. Reconnect it, refresh devices, then start live monitoring again.",
            Self::OutputUnavailable => "Speakers disconnected or unavailable. Reconnect them, refresh devices, then start live monitoring again.",
            Self::InputError => "Microphone stream failed. Refresh devices, choose a microphone, then start live monitoring again.",
            Self::OutputError => "Speaker stream failed. Refresh devices, choose speakers, then start live monitoring again.",
            Self::WorkerFailed => "Audio processing stopped unexpectedly. Start live monitoring again; if it repeats, restart Hush.",
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct StopReport {
    pub reason: StopReason,
    pub stats: Option<LiveStats>,
    /// True only after stream destruction and worker completion, including aborted startup.
    pub devices_closed: bool,
}

impl StopReport {
    #[cfg(test)]
    pub fn status_message(self) -> String {
        let mut text = if !self.devices_closed && self.reason == StopReason::Requested {
            "Stopping live monitoring."
        } else {
            self.reason.message()
        }
        .to_owned();
        if !self.devices_closed {
            text.push_str(" The audio device is still closing.");
        }
        if let Some(stats) = self.stats {
            text.push_str(&format!(" Drops in/out: {}/{}; underrun samples: {}; buffering samples: {}; clock correction: {} ppm.",
                stats.input_dropped_samples, stats.output_dropped_samples, stats.output_underrun_samples,
                stats.output_buffering_samples, stats.clock_correction_ppm));
        }
        text
    }
}

#[derive(Default)]
pub(super) struct StopSignal {
    // 0 = running. Requested may become a failure; the first failure is retained.
    reason: AtomicU8,
    closed: AtomicBool,
    watchdog: AtomicBool,
}

impl StopSignal {
    pub fn request_stop(&self) {
        let _ = self.reason.compare_exchange(
            0,
            StopReason::Requested as u8,
            Ordering::AcqRel,
            Ordering::Acquire,
        );
    }

    fn fail(&self, reason: StopReason) {
        if self
            .reason
            .compare_exchange(0, reason as u8, Ordering::AcqRel, Ordering::Acquire)
            == Err(StopReason::Requested as u8)
        {
            let _ = self.reason.compare_exchange(
                StopReason::Requested as u8,
                reason as u8,
                Ordering::AcqRel,
                Ordering::Acquire,
            );
        }
    }

    /// Called by CPAL error callbacks: at most two atomic comparisons, no allocation/logging.
    pub fn stream_error(&self, input: bool, error: &cpal::StreamError) {
        let unavailable = matches!(error, cpal::StreamError::DeviceNotAvailable);
        self.fail(match (input, unavailable) {
            (true, true) => StopReason::InputUnavailable,
            (true, false) => StopReason::InputError,
            (false, true) => StopReason::OutputUnavailable,
            (false, false) => StopReason::OutputError,
        });
    }

    pub fn worker_failed(&self) {
        self.fail(StopReason::WorkerFailed);
    }

    pub fn is_stopping(&self) -> bool {
        self.reason.load(Ordering::Acquire) != 0
    }

    fn reason(&self) -> Option<StopReason> {
        match self.reason.load(Ordering::Acquire) {
            0 => None,
            1 => Some(StopReason::Requested),
            2 => Some(StopReason::InputUnavailable),
            3 => Some(StopReason::InputError),
            4 => Some(StopReason::OutputUnavailable),
            5 => Some(StopReason::OutputError),
            6 => Some(StopReason::WorkerFailed),
            _ => unreachable!("invalid stop reason"),
        }
    }

    pub fn startup_failure(&self, error: &eyre::Report) -> Option<String> {
        match self.reason() {
            None => Some(format!("{error:#}")),
            Some(StopReason::Requested) => None,
            Some(reason) => Some(reason.message().into()),
        }
    }

    pub fn mark_closed(&self) {
        self.closed.store(true, Ordering::Release);
    }

    pub fn is_closed(&self) -> bool {
        self.closed.load(Ordering::Acquire)
    }

    pub fn begin_watchdog(&self) -> bool {
        !self.watchdog.swap(true, Ordering::AcqRel)
    }

    pub fn report(&self, stats: Option<LiveStats>) -> StopReport {
        StopReport {
            reason: self.reason().unwrap_or(StopReason::Requested),
            stats,
            devices_closed: self.is_closed(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn callback_failure_during_startup_is_reported() {
        let signal = StopSignal::default();
        signal.stream_error(true, &cpal::StreamError::DeviceNotAvailable);
        assert!(signal.is_stopping());
        let message = signal
            .startup_failure(&eyre::eyre!("Live monitoring was stopped"))
            .unwrap();
        assert!(message.contains("Microphone") && message.contains("refresh devices"));
    }

    #[test]
    fn stop_and_later_callbacks_preserve_first_failure() {
        for stop_first in [false, true] {
            let signal = StopSignal::default();
            if stop_first {
                signal.request_stop();
            }
            signal.stream_error(false, &cpal::StreamError::DeviceNotAvailable);
            signal.request_stop();
            signal.stream_error(true, &cpal::StreamError::DeviceNotAvailable);
            signal.worker_failed();
            assert_eq!(signal.report(None).reason, StopReason::OutputUnavailable);
        }
    }

    #[test]
    fn final_stats_do_not_hide_device_failure() {
        let signal = StopSignal::default();
        signal.stream_error(false, &cpal::StreamError::DeviceNotAvailable);
        let waiting = signal.report(None);
        assert!(!waiting.devices_closed);
        assert!(waiting.status_message().contains("still closing"));
        signal.mark_closed();
        let final_report = signal.report(Some(LiveStats {
            frames_processed: 12,
            ..LiveStats::default()
        }));
        assert!(final_report.devices_closed);
        assert_eq!(final_report.stats.unwrap().frames_processed, 12);
        let message = final_report.status_message();
        assert!(message.contains("Speakers disconnected") && message.contains("Drops in/out"));
        assert!(!message.contains("still closing"));
    }

    #[test]
    fn requested_startup_cancel_is_distinct_from_failure() {
        let signal = StopSignal::default();
        assert_eq!(
            signal
                .startup_failure(&eyre::eyre!("missing model"))
                .unwrap(),
            "missing model"
        );
        signal.request_stop();
        assert!(signal.startup_failure(&eyre::eyre!("cancelled")).is_none());
        assert!(signal.begin_watchdog());
        assert!(!signal.begin_watchdog());
        assert!(signal
            .report(None)
            .status_message()
            .starts_with("Stopping live monitoring."));
        signal.mark_closed();
        assert!(signal.report(None).devices_closed);
        assert_eq!(signal.report(None).reason, StopReason::Requested);
        assert!(signal
            .report(None)
            .status_message()
            .starts_with("Live monitoring stopped."));
    }

    #[test]
    fn worker_failure_stops_monitoring() {
        let signal = StopSignal::default();
        signal.worker_failed();
        assert!(signal.is_stopping());
        assert_eq!(signal.report(None).reason, StopReason::WorkerFailed);
    }
}
