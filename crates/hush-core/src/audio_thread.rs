//! Scheduling for audio threads, without special rights.
//!
//! - [`periodic`]: a short task that wakes at a fixed period (the virtual microphone feeder,
//!   simulated device callbacks). macOS: Mach real-time policy. Windows: MMCSS "Audio".
//! - [`interactive`]: compute-heavy inference threads. macOS: user-interactive QoS, which keeps
//!   them on performance cores and away from timer coalescing. Elsewhere: nothing; on Windows
//!   use [`crate::WorkerScheduling::WindowsAudio`].
//!
//! Failures keep normal scheduling.

/// Keeps a [`periodic`] registration until dropped, on the thread that made it.
pub struct AudioThread {
    #[cfg(target_os = "windows")]
    _registration: Option<crate::mmcss::Registration>,
    _thread_bound: std::marker::PhantomData<std::rc::Rc<()>>,
}

/// Schedule the calling thread for a short task every `period` that needs at most a tenth of
/// it in CPU time.
pub fn periodic(period: std::time::Duration) -> AudioThread {
    #[cfg(target_os = "macos")]
    macos::time_constraint(period);
    #[cfg(not(target_os = "macos"))]
    let _ = period;
    AudioThread {
        #[cfg(target_os = "windows")]
        _registration: crate::mmcss::Registration::enter().ok(),
        _thread_bound: std::marker::PhantomData,
    }
}

/// Mark the calling thread as latency-critical compute work (macOS QoS).
pub fn interactive() {
    #[cfg(target_os = "macos")]
    // SAFETY: changes only the calling thread's QoS class.
    unsafe {
        libc::pthread_set_qos_class_self_np(libc::qos_class_t::QOS_CLASS_USER_INTERACTIVE, 0);
    }
}

#[cfg(target_os = "macos")]
mod macos {
    // libc marks the Mach calls deprecated in favour of the mach2 crate; they are still the
    // system calls Core Audio clients use, and one more dependency is not worth it.
    #[allow(deprecated)]
    pub fn time_constraint(period: std::time::Duration) {
        super::interactive();
        // SAFETY: these calls change only the calling thread's scheduling.
        unsafe {
            let mut timebase = libc::mach_timebase_info { numer: 0, denom: 0 };
            if libc::mach_timebase_info(&mut timebase) != 0 || timebase.numer == 0 {
                return;
            }
            let ticks = |ns: u128| {
                (ns * u128::from(timebase.denom) / u128::from(timebase.numer)).min(u32::MAX.into())
                    as u32
            };
            let period = period.as_nanos();
            let mut policy = libc::thread_time_constraint_policy {
                period: ticks(period),
                computation: ticks(period / 10),
                constraint: ticks(period / 2),
                preemptible: 1,
            };
            libc::thread_policy_set(
                libc::mach_thread_self(),
                libc::THREAD_TIME_CONSTRAINT_POLICY as libc::thread_policy_flavor_t,
                std::ptr::addr_of_mut!(policy).cast(),
                libc::THREAD_TIME_CONSTRAINT_POLICY_COUNT,
            );
        }
    }
}
