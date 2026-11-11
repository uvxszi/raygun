//! What JavaScript runtimes may hold, and what they hold (ARCHITECTURE §7.2).
//!
//! Each runtime has its own memory limit, which QuickJS keeps, sized by the device's memory and
//! by what runs in it ([`Budget`]): an extension written for Raygun leaves pages in Rust
//! (`raygun.html`), while a converted plugin reads whole pages and their data as strings. All of
//! one host's runtimes also share a [`HeapPool`]: past it, the runtime asking for more fails
//! rather than the device running short. Every runtime allocates through a [`Metered`]
//! allocator, which counts what it holds now and the most it held, so a call's high-water mark is
//! known, and a runtime that holds a lot after a call is collected (`js.rs`).

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use rquickjs::allocator::{Allocator, RustAllocator};

const MIB: usize = 1024 * 1024;
/// The device's memory when it doesn't say: a mid-range phone's.
const ASSUMED_MEMORY: u64 = 4 * 1024 * MIB as u64;

/// How much JavaScript runtimes may hold on a device.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Budget {
    /// One extension written for Raygun.
    pub extension: usize,
    /// One converted plugin or script (Nuclear, Miru, LNReader, Mihon): code written for a
    /// desktop player or a browser holds whole pages and their parsed data.
    pub converted: usize,
    /// Every runtime of a host together.
    pub all: usize,
}

impl Budget {
    /// The budget for a device with `total` bytes of memory (unknown: a 4 GB phone's): a share
    /// of it, within bounds that keep small devices safe and big ones from paying for nothing.
    pub fn for_memory(total: Option<u64>) -> Self {
        let total = usize::try_from(total.unwrap_or(ASSUMED_MEMORY)).unwrap_or(usize::MAX);
        Self {
            extension: (total / 64).clamp(64 * MIB, 128 * MIB),
            converted: (total / 24).clamp(96 * MIB, 256 * MIB),
            all: (total / 8).clamp(256 * MIB, 1024 * MIB),
        }
    }

    /// This device's budget.
    pub fn of_device() -> Self {
        Self::for_memory(device_memory())
    }
}

/// This device's memory in bytes, when it tells.
pub fn device_memory() -> Option<u64> {
    #[cfg(any(target_os = "linux", target_os = "android"))]
    {
        let info = std::fs::read_to_string("/proc/meminfo").ok()?;
        meminfo_total(&info)
    }
    #[cfg(windows)]
    {
        windows_memory()
    }
    #[cfg(not(any(target_os = "linux", target_os = "android", windows)))]
    {
        None
    }
}

/// `MemTotal` of a `/proc/meminfo`, in bytes.
#[cfg_attr(
    not(any(target_os = "linux", target_os = "android", test)),
    allow(dead_code)
)]
fn meminfo_total(info: &str) -> Option<u64> {
    let line = info.lines().find(|line| line.starts_with("MemTotal:"))?;
    let kilobytes: u64 = line
        .trim_start_matches("MemTotal:")
        .trim()
        .trim_end_matches("kB")
        .trim()
        .parse()
        .ok()?;
    kilobytes.checked_mul(1024)
}

#[cfg(windows)]
fn windows_memory() -> Option<u64> {
    /// `MEMORYSTATUSEX`.
    #[repr(C)]
    struct MemoryStatus {
        length: u32,
        memory_load: u32,
        total_physical: u64,
        available_physical: u64,
        total_page_file: u64,
        available_page_file: u64,
        total_virtual: u64,
        available_virtual: u64,
        available_extended_virtual: u64,
    }
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GlobalMemoryStatusEx(status: *mut MemoryStatus) -> i32;
    }
    let mut status = MemoryStatus {
        length: std::mem::size_of::<MemoryStatus>() as u32,
        memory_load: 0,
        total_physical: 0,
        available_physical: 0,
        total_page_file: 0,
        available_page_file: 0,
        total_virtual: 0,
        available_virtual: 0,
        available_extended_virtual: 0,
    };
    // SAFETY: `status` is a `MEMORYSTATUSEX` with its length set, as the call requires.
    let answered = unsafe { GlobalMemoryStatusEx(&mut status) } != 0;
    (answered && status.total_physical > 0).then_some(status.total_physical)
}

/// What a host's runtimes hold together, and the most they may.
#[derive(Debug, Clone)]
pub struct HeapPool(Arc<PoolState>);

#[derive(Debug)]
struct PoolState {
    held: AtomicUsize,
    limit: usize,
}

impl HeapPool {
    pub fn new(limit: usize) -> Self {
        Self(Arc::new(PoolState {
            held: AtomicUsize::new(0),
            limit,
        }))
    }

    /// Bytes every runtime in the pool holds.
    pub fn held(&self) -> usize {
        self.0.held.load(Ordering::Relaxed)
    }

    pub fn limit(&self) -> usize {
        self.0.limit
    }

    /// Whether `bytes` more still fit.
    fn admits(&self, bytes: usize) -> bool {
        self.held().saturating_add(bytes) <= self.0.limit
    }
}

impl Default for HeapPool {
    fn default() -> Self {
        Self::new(Budget::of_device().all)
    }
}

/// What one runtime's heap holds.
#[derive(Debug, Default)]
pub struct HeapMeter {
    now: AtomicUsize,
    peak: AtomicUsize,
    /// Allocations the pool refused it (other runtimes held the rest).
    refused: AtomicUsize,
}

/// A runtime's heap as read at one moment.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct HeapUse {
    /// Bytes it holds now.
    pub now: usize,
    /// The most it held since its peak was last taken.
    pub peak: usize,
    /// The most it may hold.
    pub limit: usize,
}

impl HeapMeter {
    fn grew(&self, bytes: usize) {
        let now = self.now.fetch_add(bytes, Ordering::Relaxed) + bytes;
        self.peak.fetch_max(now, Ordering::Relaxed);
    }

    fn shrank(&self, bytes: usize) {
        self.now.fetch_sub(bytes, Ordering::Relaxed);
    }

    /// Bytes held now.
    pub fn now(&self) -> usize {
        self.now.load(Ordering::Relaxed)
    }

    /// The most held since the peak was last taken.
    pub fn peak(&self) -> usize {
        self.peak.load(Ordering::Relaxed).max(self.now())
    }

    /// The most held since the peak was last taken; the next peak starts from what's held now.
    pub fn take_peak(&self) -> usize {
        let now = self.now();
        self.peak.swap(now, Ordering::Relaxed).max(now)
    }

    /// How many allocations the pool refused this runtime so far.
    pub fn refused(&self) -> usize {
        self.refused.load(Ordering::Relaxed)
    }
}

/// Rust's allocator, counting into a runtime's [`HeapMeter`] and its host's [`HeapPool`]. QuickJS
/// keeps the runtime within its own memory limit; this refuses what would take the pool past
/// its limit (QuickJS then throws "out of memory" in the runtime that asked).
pub(crate) struct Metered {
    pub meter: Arc<HeapMeter>,
    pub pool: HeapPool,
}

impl Metered {
    fn grew(&self, bytes: usize) {
        self.meter.grew(bytes);
        self.pool.0.held.fetch_add(bytes, Ordering::Relaxed);
    }

    fn shrank(&self, bytes: usize) {
        self.meter.shrank(bytes);
        self.pool.0.held.fetch_sub(bytes, Ordering::Relaxed);
    }

    /// Whether the pool has room for `bytes` more; a refusal is counted.
    fn admits(&self, bytes: usize) -> bool {
        let admits = self.pool.admits(bytes);
        if !admits {
            self.meter.refused.fetch_add(1, Ordering::Relaxed);
        }
        admits
    }
}

// SAFETY: every call goes to `RustAllocator` with the pointers it handed out; the meter only
// reads the sizes it records, and a refusal is a null pointer, as QuickJS expects of `malloc`.
unsafe impl Allocator for Metered {
    fn alloc(&mut self, size: usize) -> *mut u8 {
        if !self.admits(size) {
            return std::ptr::null_mut();
        }
        let pointer = RustAllocator.alloc(size);
        if !pointer.is_null() {
            // SAFETY: `pointer` was just allocated by `RustAllocator`.
            self.grew(unsafe { RustAllocator::usable_size(pointer) });
        }
        pointer
    }

    fn calloc(&mut self, count: usize, size: usize) -> *mut u8 {
        if !self.admits(count.saturating_mul(size)) {
            return std::ptr::null_mut();
        }
        let pointer = RustAllocator.calloc(count, size);
        if !pointer.is_null() {
            // SAFETY: `pointer` was just allocated by `RustAllocator`.
            self.grew(unsafe { RustAllocator::usable_size(pointer) });
        }
        pointer
    }

    unsafe fn dealloc(&mut self, pointer: *mut u8) {
        // SAFETY: the caller hands back a pointer this allocator gave out.
        unsafe {
            self.shrank(RustAllocator::usable_size(pointer));
            RustAllocator.dealloc(pointer);
        }
    }

    unsafe fn realloc(&mut self, pointer: *mut u8, new_size: usize) -> *mut u8 {
        if pointer.is_null() {
            return self.alloc(new_size);
        }
        // SAFETY: the caller hands back a pointer this allocator gave out; a null answer leaves
        // it as it was.
        unsafe {
            let old = RustAllocator::usable_size(pointer);
            if new_size > old && !self.admits(new_size - old) {
                return std::ptr::null_mut();
            }
            let moved = RustAllocator.realloc(pointer, new_size);
            if !moved.is_null() {
                let new = RustAllocator::usable_size(moved);
                if new >= old {
                    self.grew(new - old);
                } else {
                    self.shrank(old - new);
                }
            }
            moved
        }
    }

    unsafe fn usable_size(pointer: *mut u8) -> usize {
        // SAFETY: as the caller promises, `pointer` came from this allocator.
        unsafe { RustAllocator::usable_size(pointer) }
    }
}

/// Megabytes, for logs.
pub(crate) fn megabytes(bytes: usize) -> f64 {
    bytes as f64 / (1024.0 * 1024.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    const GIB: u64 = 1024 * 1024 * 1024;

    #[test]
    fn budgets_follow_the_device_within_bounds() {
        let budget = |gib: u64| {
            let budget = Budget::for_memory(Some(gib * GIB));
            (
                budget.extension / MIB,
                budget.converted / MIB,
                budget.all / MIB,
            )
        };
        // A small phone keeps the floors; the floors never fall below what pages need.
        assert_eq!(budget(2), (64, 96, 256));
        assert_eq!(budget(4), (64, 170, 512));
        assert_eq!(budget(6), (96, 256, 768));
        // Past that, more memory buys nothing: the ceilings hold on any desktop.
        assert_eq!(budget(16), (128, 256, 1024));
        assert_eq!(Budget::for_memory(None), Budget::for_memory(Some(4 * GIB)));
        assert!(
            device_memory().is_none_or(|bytes| bytes > 256 * MIB as u64),
            "{:?}",
            device_memory()
        );
    }

    #[test]
    fn meminfo_gives_the_total() {
        let info = "MemTotal:        3884764 kB\nMemFree:          144436 kB\n";
        assert_eq!(meminfo_total(info), Some(3_884_764 * 1024));
        assert_eq!(meminfo_total("MemFree: 1 kB"), None);
    }

    #[test]
    fn the_pool_refuses_what_would_pass_its_limit_and_meters_count() {
        let pool = HeapPool::new(64 * 1024);
        let meter = Arc::new(HeapMeter::default());
        let mut allocator = Metered {
            meter: meter.clone(),
            pool: pool.clone(),
        };
        let first = allocator.alloc(40 * 1024);
        assert!(!first.is_null());
        assert!(pool.held() >= 40 * 1024);
        assert!(allocator.alloc(40 * 1024).is_null(), "past the pool");
        // SAFETY: `first` came from this allocator.
        let grown = unsafe { allocator.realloc(first, 80 * 1024) };
        assert!(
            grown.is_null(),
            "growing past the pool fails and keeps the block"
        );
        // SAFETY: `first` is still this allocator's.
        unsafe { allocator.dealloc(first) };
        assert_eq!((pool.held(), meter.now()), (0, 0));
        assert!(meter.take_peak() >= 40 * 1024);
        assert_eq!(meter.peak(), 0, "the next peak starts from what's held");
    }
}
