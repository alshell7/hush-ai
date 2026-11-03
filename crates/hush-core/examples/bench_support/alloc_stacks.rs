//! Optional allocation-site capture for the Windows development example only.
//! The allocator path uses fixed storage and atomics; symbolization runs in a separate process.
use std::{
    cell::Cell,
    ffi::c_void,
    sync::atomic::{AtomicBool, AtomicUsize, Ordering},
};

const DEPTH: usize = 16;
const CAPACITY: usize = 1024;
static ENABLED: AtomicBool = AtomicBool::new(false);
static DROPPED: AtomicUsize = AtomicUsize::new(0);
static REENTRANT: AtomicUsize = AtomicUsize::new(0);
thread_local! { static CAPTURING: Cell<bool> = const { Cell::new(false) }; }

struct Slot {
    // 0 = empty, 1 = being initialized, otherwise the stack/request-size hash.
    key: AtomicUsize,
    size: AtomicUsize,
    frames: [AtomicUsize; DEPTH],
    count: AtomicUsize,
}
static SLOTS: [Slot; CAPACITY] = [const {
    Slot {
        key: AtomicUsize::new(0),
        size: AtomicUsize::new(0),
        frames: [const { AtomicUsize::new(0) }; DEPTH],
        count: AtomicUsize::new(0),
    }
}; CAPACITY];

#[link(name = "kernel32")]
unsafe extern "system" {
    fn RtlCaptureStackBackTrace(
        skip: u32,
        count: u32,
        frames: *mut *mut c_void,
        hash: *mut u32,
    ) -> u16;
    fn GetModuleHandleW(name: *const u16) -> *mut c_void;
}

pub fn enable(value: bool) {
    ENABLED.store(value, Ordering::Relaxed);
}

#[inline(never)]
pub fn record(size: usize) {
    if !ENABLED.load(Ordering::Relaxed) {
        return;
    }
    CAPTURING.with(|active| {
        if active.replace(true) {
            REENTRANT.fetch_add(1, Ordering::Relaxed);
            return;
        }
        let mut addresses = [std::ptr::null_mut(); DEPTH];
        // SAFETY: the stack array holds DEPTH writable pointers. No symbol APIs are called
        // here. The TLS guard accounts for (and prevents recursion from) nested allocations.
        unsafe {
            RtlCaptureStackBackTrace(
                1,
                DEPTH as u32,
                addresses.as_mut_ptr(),
                std::ptr::null_mut(),
            );
        }
        let mut hash = size.wrapping_add(0xcbf29ce484222325);
        for address in addresses {
            hash = (hash ^ address as usize).wrapping_mul(0x100000001b3);
        }
        hash = hash.max(2);
        for offset in 0..CAPACITY {
            let slot = &SLOTS[hash.wrapping_add(offset) % CAPACITY];
            let mut key = slot.key.load(Ordering::Acquire);
            if key == 0 {
                match slot
                    .key
                    .compare_exchange(0, 1, Ordering::AcqRel, Ordering::Acquire)
                {
                    Ok(_) => {
                        slot.size.store(size, Ordering::Relaxed);
                        for (frame, address) in slot.frames.iter().zip(addresses) {
                            frame.store(address as usize, Ordering::Relaxed);
                        }
                        slot.count.store(1, Ordering::Relaxed);
                        slot.key.store(hash, Ordering::Release);
                        active.set(false);
                        return;
                    }
                    Err(value) => key = value,
                }
            }
            while key == 1 {
                std::hint::spin_loop();
                key = slot.key.load(Ordering::Acquire);
            }
            if key == hash
                && slot.size.load(Ordering::Relaxed) == size
                && slot
                    .frames
                    .iter()
                    .zip(addresses)
                    .all(|(frame, address)| frame.load(Ordering::Relaxed) == address as usize)
            {
                slot.count.fetch_add(1, Ordering::Relaxed);
                active.set(false);
                return;
            }
        }
        DROPPED.fetch_add(1, Ordering::Relaxed);
        active.set(false);
    });
}

/// Called only after inference has joined and the counting allocator is disabled.
pub fn report() -> serde_json::Value {
    let traces: Vec<_> = SLOTS
        .iter()
        .filter_map(|slot| {
            if slot.key.load(Ordering::Acquire) < 2 {
                return None;
            }
            let addresses: Vec<_> = slot
                .frames
                .iter()
                .map(|f| f.load(Ordering::Relaxed))
                .take_while(|a| *a != 0)
                .map(|a| format!("0x{a:x}"))
                .collect();
            Some(serde_json::json!({
                "request_bytes": slot.size.load(Ordering::Relaxed),
                "requests": slot.count.load(Ordering::Relaxed),
                "addresses": addresses,
            }))
        })
        .collect();
    // SAFETY: null requests the executable's already-loaded module handle; no ownership change.
    let base = unsafe { GetModuleHandleW(std::ptr::null()) } as usize;
    serde_json::json!({
        "executable": std::env::current_exe().unwrap(),
        "module_base": format!("0x{base:x}"),
        "depth": DEPTH, "capacity": CAPACITY,
        "dropped_requests": DROPPED.load(Ordering::Relaxed),
        "reentrant_requests": REENTRANT.load(Ordering::Relaxed),
        "traces": traces,
        "notes": "Successful Rust allocation/reallocation requests only. Fixed-size stack table; truncated stacks and external/native allocations are not complete heap coverage. No timing claim. Keep matching release PDB."
    })
}
