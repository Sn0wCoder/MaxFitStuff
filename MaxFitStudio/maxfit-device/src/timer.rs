//! A frame pacer with sub-millisecond accuracy.
//!
//! `std::thread::sleep` on Windows is driven by the ~15.6 ms system tick unless
//! something has raised the global timer resolution, so a naive 60 Hz loop
//! actually runs near 32 Hz with heavy jitter — and it looks like a USB
//! throughput problem rather than a clock problem. A high-resolution waitable
//! timer gives ~0.5 ms granularity with no process-wide side effects and no
//! battery penalty, unlike `timeBeginPeriod`.
//!
//! The three kernel32 entry points are declared directly rather than pulling in
//! the `windows` crate: that keeps this crate free of `raw-dylib` imports, which
//! on this machine require a `dlltool` that only ships with the MinGW toolchain.

use std::ffi::c_void;
use std::ptr;
use std::time::{Duration, Instant};

#[cfg(windows)]
#[link(name = "kernel32")]
unsafe extern "system" {
    fn CreateWaitableTimerExW(
        attributes: *const c_void,
        name: *const u16,
        flags: u32,
        desired_access: u32,
    ) -> *mut c_void;
    fn SetWaitableTimer(
        timer: *mut c_void,
        due_time: *const i64,
        period_ms: i32,
        completion_routine: *const c_void,
        completion_arg: *const c_void,
        resume: i32,
    ) -> i32;
    fn WaitForSingleObject(handle: *mut c_void, millis: u32) -> u32;
    fn CloseHandle(handle: *mut c_void) -> i32;
}

#[cfg(windows)]
const CREATE_WAITABLE_TIMER_HIGH_RESOLUTION: u32 = 0x0000_0002;
#[cfg(windows)]
const TIMER_ALL_ACCESS: u32 = 0x001F_0003;

/// Spin rather than sleep for the last stretch, where the timer's own
/// granularity would otherwise overshoot the deadline.
const SPIN_GUARD: Duration = Duration::from_micros(300);

struct HiResTimer {
    #[cfg(windows)]
    handle: *mut c_void,
}

// The handle is only ever touched from the thread that owns the Pacer.
unsafe impl Send for HiResTimer {}

impl HiResTimer {
    fn new() -> Option<Self> {
        #[cfg(windows)]
        {
            let handle = unsafe {
                CreateWaitableTimerExW(
                    ptr::null(),
                    ptr::null(),
                    CREATE_WAITABLE_TIMER_HIGH_RESOLUTION,
                    TIMER_ALL_ACCESS,
                )
            };
            if handle.is_null() {
                return None; // Pre-1803 Windows; caller falls back to sleep.
            }
            Some(Self { handle })
        }
        #[cfg(not(windows))]
        {
            None
        }
    }

    #[cfg(windows)]
    fn sleep(&self, d: Duration) {
        // Negative due time means relative, in 100-nanosecond units.
        let due = -((d.as_nanos() / 100) as i64);
        if due == 0 {
            return;
        }
        unsafe {
            if SetWaitableTimer(self.handle, &due, 0, ptr::null(), ptr::null(), 0) != 0 {
                WaitForSingleObject(self.handle, u32::MAX);
            }
        }
    }

    #[cfg(not(windows))]
    fn sleep(&self, d: Duration) {
        std::thread::sleep(d);
    }
}

impl Drop for HiResTimer {
    fn drop(&mut self) {
        #[cfg(windows)]
        unsafe {
            CloseHandle(self.handle);
        }
    }
}

/// Paces a loop to a target frame rate.
///
/// Falling behind resets the phase instead of bursting to catch up, so a slow
/// frame can never cascade into a backlog of queued writes.
pub struct Pacer {
    timer: Option<HiResTimer>,
    budget: Duration,
    next: Instant,
    pub dropped: u64,
}

impl Pacer {
    pub fn new(fps: u16) -> Self {
        Self {
            timer: HiResTimer::new(),
            budget: Self::budget_for(fps),
            next: Instant::now(),
            dropped: 0,
        }
    }

    fn budget_for(fps: u16) -> Duration {
        Duration::from_secs_f32(1.0 / f32::from(fps.clamp(1, 240)))
    }

    pub fn set_fps(&mut self, fps: u16) {
        self.budget = Self::budget_for(fps);
        // Re-phase so the change takes effect on the next frame rather than
        // after a long stale deadline.
        self.next = Instant::now();
    }

    pub fn budget(&self) -> Duration {
        self.budget
    }

    /// Sleeps until the next frame deadline.
    pub fn wait_next(&mut self) {
        self.next += self.budget;
        let now = Instant::now();
        if self.next <= now {
            // Already late: drop the frame and restart the phase here.
            self.dropped += 1;
            self.next = now + self.budget;
            return;
        }
        self.sleep_until(self.next);
    }

    fn sleep_until(&self, deadline: Instant) {
        if let Some(t) = &self.timer {
            let now = Instant::now();
            if let Some(d) = deadline.checked_duration_since(now)
                && d > SPIN_GUARD
            {
                t.sleep(d - SPIN_GUARD);
            }
        } else if let Some(d) = deadline.checked_duration_since(Instant::now())
            && d > SPIN_GUARD
        {
            std::thread::sleep(d - SPIN_GUARD);
        }
        // Burn the remainder so the deadline is hit precisely.
        while Instant::now() < deadline {
            std::hint::spin_loop();
        }
    }
}
