//! Optional inference-worker placement and scheduling. No process-wide changes.
use eyre::Result;

/// Optional OS scheduling for dedicated inference workers. Configure outside callbacks.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum WorkerScheduling {
    /// Ordinary OS scheduling on every platform.
    #[default]
    OsDefault,
    /// Experimental Windows MMCSS Audio task at normal relative priority.
    /// Requires unrestricted placement; registration failure is returned to the caller.
    /// This may reduce preemption but does not guarantee audio deadlines.
    WindowsAudio,
}

/// A scoped scheduling registration. It cannot move to another thread.
/// Dropping it restores the registering thread's previous scheduling characteristics.
///
/// ```compile_fail
/// let guard = hush_core::WorkerScheduling::OsDefault.enter_current_thread().unwrap();
/// std::thread::spawn(move || drop(guard)); // registration cannot cross threads
/// ```
#[derive(Default)]
pub struct WorkerSchedulingGuard {
    #[cfg(target_os = "windows")]
    _registration: Option<crate::mmcss::Registration>,
    _thread_bound: std::marker::PhantomData<std::rc::Rc<()>>,
}

impl WorkerScheduling {
    /// Register the current inference coordinator until the returned guard is dropped.
    /// `LiveWorker` does this automatically. Direct callers can use this around their
    /// processing loop; never register inside an audio callback or on the GUI thread.
    pub fn enter_current_thread(self) -> Result<WorkerSchedulingGuard> {
        match self {
            Self::OsDefault => Ok(WorkerSchedulingGuard::default()),
            Self::WindowsAudio => {
                #[cfg(target_os = "windows")]
                return Ok(WorkerSchedulingGuard {
                    _registration: Some(crate::mmcss::Registration::enter()?),
                    ..Default::default()
                });
                #[cfg(not(target_os = "windows"))]
                eyre::bail!("Windows Audio scheduling requires Windows");
            }
        }
    }
}

/// Scheduling policy for a dedicated inference pool.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum WorkerPlacement {
    /// Leave scheduling to the OS (the default on every platform).
    #[default]
    Unrestricted,
    /// Experimental: allow one logical processor per available physical core.
    /// Supported on 64-bit Windows with one processor group. This may be slower on some CPUs.
    PhysicalCores,
}

pub(crate) fn build(
    threads: usize,
    placement: WorkerPlacement,
    scheduling: WorkerScheduling,
) -> Result<Option<rayon::ThreadPool>> {
    if scheduling == WorkerScheduling::WindowsAudio {
        eyre::ensure!(
            placement == WorkerPlacement::Unrestricted,
            "Windows Audio scheduling requires unrestricted worker placement"
        );
        #[cfg(target_os = "windows")]
        // Rayon resolves zero to its default count without starting an unused global pool.
        return crate::mmcss::pool(threads).map(Some);
        #[cfg(not(target_os = "windows"))]
        eyre::bail!("Windows Audio scheduling requires Windows");
    }
    match placement {
        WorkerPlacement::Unrestricted if threads == 0 => Ok(None),
        WorkerPlacement::Unrestricted => {
            let pool = rayon::ThreadPoolBuilder::new()
                .num_threads(threads)
                .start_handler(|_| crate::audio_thread::interactive())
                .build()?;
            Ok(Some(pool))
        }
        WorkerPlacement::PhysicalCores => physical_pool(threads).map(Some),
    }
}

#[cfg(not(all(target_os = "windows", target_pointer_width = "64")))]
fn physical_pool(_: usize) -> Result<rayon::ThreadPool> {
    eyre::bail!("Physical-core worker placement currently requires 64-bit Windows");
}

#[cfg(all(target_os = "windows", target_pointer_width = "64"))]
fn physical_pool(threads: usize) -> Result<rayon::ThreadPool> {
    let mask = windows::physical_mask()?;
    let available = mask.count_ones() as usize;
    let threads = if threads == 0 { available } else { threads };
    eyre::ensure!(
        threads <= available,
        "Requested {threads} workers but only {available} physical cores are allowed"
    );
    // A start-handler failure must reach the caller before the new pool is installed.
    // Each worker uses the same allowed set; Windows still schedules workers within that set.
    let (sender, receiver) = std::sync::mpsc::channel();
    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(threads)
        .start_handler(move |_| {
            let _ = sender.send(windows::apply_mask(mask));
        })
        .build()?;
    for _ in 0..pool.current_num_threads() {
        receiver.recv()??;
    }
    Ok(pool)
}

// Offsets are for SYSTEM_LOGICAL_PROCESSOR_INFORMATION_EX / PROCESSOR_RELATIONSHIP /
// GROUP_AFFINITY on Win64. Decode checked byte slices rather than casting variable-size records.
// https://learn.microsoft.com/windows/win32/api/winnt/ns-winnt-processor_relationship
#[cfg(any(test, all(target_os = "windows", target_pointer_width = "64")))]
fn select_physical_mask(mut records: &[u8], allowed: u64) -> Result<u64> {
    let mut selected = 0u64;
    let mut seen = 0u64;
    while !records.is_empty() {
        eyre::ensure!(records.len() >= 48, "Truncated Windows CPU topology record");
        let relationship = u32::from_le_bytes(records[0..4].try_into()?);
        let size = u32::from_le_bytes(records[4..8].try_into()?) as usize;
        eyre::ensure!(
            size >= 48 && size <= records.len(),
            "Invalid CPU topology size"
        );
        eyre::ensure!(
            relationship == 0,
            "Expected a physical-core topology record"
        );
        let groups = u16::from_le_bytes(records[30..32].try_into()?);
        let group = u16::from_le_bytes(records[40..42].try_into()?);
        eyre::ensure!(
            groups == 1 && group == 0,
            "Multiple processor groups are unsupported"
        );
        let core = u64::from_le_bytes(records[32..40].try_into()?);
        eyre::ensure!(
            core != 0 && core & seen == 0,
            "Invalid or overlapping core masks"
        );
        seen |= core;
        let available = core & allowed;
        // Works for bit 63 as well as smaller masks; no signed conversions.
        if available != 0 {
            selected |= 1u64 << available.trailing_zeros();
        }
        records = &records[size..];
    }
    eyre::ensure!(
        selected != 0,
        "No physical cores are allowed by the process affinity"
    );
    Ok(selected)
}

#[cfg(all(target_os = "windows", target_pointer_width = "64"))]
mod windows {
    use super::*;
    use std::{ffi::c_void, io, ptr};

    #[link(name = "kernel32")]
    extern "system" {
        fn GetActiveProcessorGroupCount() -> u16;
        fn GetCurrentProcess() -> *mut c_void;
        fn GetCurrentThread() -> *mut c_void;
        fn GetProcessAffinityMask(
            process: *mut c_void,
            process_mask: *mut usize,
            system_mask: *mut usize,
        ) -> i32;
        fn GetLogicalProcessorInformationEx(
            relation: i32,
            buffer: *mut c_void,
            length: *mut u32,
        ) -> i32;
        fn SetThreadAffinityMask(thread: *mut c_void, mask: usize) -> usize;
        #[cfg(test)]
        fn GetThreadGroupAffinity(thread: *mut c_void, affinity: *mut GroupAffinity) -> i32;
    }

    pub(super) fn physical_mask() -> Result<usize> {
        // SAFETY: output pointers below refer to suitably aligned live storage; pseudo-handles
        // refer to the current process/thread and do not need closing.
        unsafe {
            eyre::ensure!(
                GetActiveProcessorGroupCount() == 1,
                "Physical-core placement supports one processor group only"
            );
            let (mut allowed, mut system) = (0, 0);
            if GetProcessAffinityMask(GetCurrentProcess(), &mut allowed, &mut system) == 0 {
                return Err(io::Error::last_os_error().into());
            }
            let mut length = 0;
            let ok = GetLogicalProcessorInformationEx(0, ptr::null_mut(), &mut length);
            let error = io::Error::last_os_error();
            eyre::ensure!(
                ok == 0 && error.raw_os_error() == Some(122) && length > 0,
                "Could not size Windows CPU topology: {error}"
            );
            // u64 allocation gives the alignment required by the Win64 union.
            let mut storage = vec![0u64; (length as usize).div_ceil(8)];
            if GetLogicalProcessorInformationEx(0, storage.as_mut_ptr().cast(), &mut length) == 0 {
                return Err(io::Error::last_os_error().into());
            }
            eyre::ensure!(
                length as usize <= storage.len() * 8,
                "Invalid topology length"
            );
            let bytes = std::slice::from_raw_parts(storage.as_ptr().cast(), length as usize);
            Ok(select_physical_mask(bytes, allowed as u64)? as usize)
        }
    }

    pub(super) fn apply_mask(mask: usize) -> Result<()> {
        // SAFETY: the pseudo-handle identifies only this newly created inference worker.
        if unsafe { SetThreadAffinityMask(GetCurrentThread(), mask) } == 0 {
            return Err(io::Error::last_os_error().into());
        }
        Ok(())
    }

    #[cfg(test)]
    #[repr(C)]
    struct GroupAffinity {
        mask: usize,
        group: u16,
        reserved: [u16; 3],
    }

    #[cfg(test)]
    pub(super) fn thread_mask() -> usize {
        let mut affinity = GroupAffinity {
            mask: 0,
            group: 0,
            reserved: [0; 3],
        };
        assert_ne!(
            unsafe { GetThreadGroupAffinity(GetCurrentThread(), &mut affinity) },
            0
        );
        affinity.mask
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn record(mask: u64) -> Vec<u8> {
        let mut bytes = vec![0; 48];
        bytes[4..8].copy_from_slice(&48u32.to_le_bytes());
        bytes[30..32].copy_from_slice(&1u16.to_le_bytes());
        bytes[32..40].copy_from_slice(&mask.to_le_bytes());
        bytes
    }

    #[test]
    fn topology_respects_siblings_restrictions_and_high_bit() {
        let records = [record(3), record(12), record(1u64 << 63)].concat();
        assert_eq!(
            select_physical_mask(&records, u64::MAX).unwrap(),
            5 | (1u64 << 63)
        );
        assert_eq!(select_physical_mask(&records, 10).unwrap(), 10);
        assert!(select_physical_mask(&records, 0).is_err());
    }

    #[test]
    fn topology_rejects_truncation_bad_size_groups_and_overlap() {
        for length in 0..48 {
            assert!(select_physical_mask(&record(3)[..length], 3).is_err());
        }
        for (offset, value) in [(0, 1), (4, 0), (4, 255), (30, 2), (40, 1)] {
            let mut bytes = record(3);
            bytes[offset] = value;
            assert!(select_physical_mask(&bytes, 3).is_err());
        }
        assert!(select_physical_mask(&[record(3), record(2)].concat(), 3).is_err());
    }

    #[cfg(all(target_os = "windows", target_pointer_width = "64"))]
    #[test]
    fn placement_applies_to_every_worker_without_changing_callers() {
        let caller_mask = windows::thread_mask();
        let expected = windows::physical_mask().unwrap();
        let pool = physical_pool(0).unwrap();
        for actual in pool.broadcast(|_| windows::thread_mask()) {
            assert_eq!(actual, expected);
        }
        assert_eq!(windows::thread_mask(), caller_mask);
        assert!(physical_pool(usize::MAX).is_err());
        drop(pool);
        assert_eq!(windows::thread_mask(), caller_mask);
    }

    #[cfg(not(all(target_os = "windows", target_pointer_width = "64")))]
    #[test]
    fn unsupported_placement_is_an_error() {
        assert!(physical_pool(0).is_err());
    }
}
