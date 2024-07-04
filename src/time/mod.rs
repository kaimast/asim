/// Contains utilities to deal with time, similar to std::time, but for simulated not real time
pub mod timer;
pub use timer::{SleepFut, Timer};

mod primitives;
pub use primitives::{Duration, Time, START_TIME};

/// Make this task wait for the specified duration
pub fn sleep(duration: Duration) -> SleepFut {
    crate::CONTEXT.with(|hdl| {
        hdl.borrow()
            .as_ref()
            .expect("Not in an asim context")
            .get_timer()
            .sleep_for(duration)
    })
}

/// Get the current simulated time
pub fn now() -> Time {
    crate::CONTEXT.with(|hdl| {
        hdl.borrow()
            .as_ref()
            .expect("Not in an asim context")
            .get_timer()
            .now()
    })
}
