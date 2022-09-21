use std::cell::RefCell;
use std::cmp::Reverse;
use std::collections::BinaryHeap;
use std::future::Future;
use std::pin::Pin;
use std::rc::Rc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::task::{Context, Poll, Waker};

use crate::time::{Duration, Time};

struct TimeEvent {
    wake_time: Time,
    waker: Waker,
}

impl PartialEq for TimeEvent {
    fn eq(&self, other: &Self) -> bool {
        self.wake_time.eq(&other.wake_time)
    }
}

impl PartialOrd for TimeEvent {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        self.wake_time.partial_cmp(&other.wake_time)
    }
}

impl Eq for TimeEvent {}

impl Ord for TimeEvent {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.wake_time.cmp(&other.wake_time)
    }
}

#[derive(Default)]
pub struct Timer {
    current_time: Rc<AtomicU64>,
    time_events: Rc<RefCell<BinaryHeap<Reverse<TimeEvent>>>>,
}

impl Timer {
    pub fn new() -> Self {
        Self::default()
    }

    /// Current simulation time (in milliseconds)
    pub fn now(&self) -> Time {
        let micros = self.current_time.load(Ordering::SeqCst);
        Time::from_micros(micros)
    }

    /// Advance time to the next event and schedule it to be run
    pub fn advance(&self) {
        let mut time_events = self.time_events.borrow_mut();
        if let Some(Reverse(time_event)) = time_events.pop() {
            // Move to the time of the next event
            self.current_time
                .store(time_event.wake_time.as_micros(), Ordering::SeqCst);
            time_event.waker.wake();
        } else {
            panic!("No time event left");
        }
    }

    #[must_use]
    pub fn sleep_for(&self, duration: Duration) -> SleepFut {
        if duration.is_zero() {
            log::warn!("sleep_for called with no delay");
        }

        let now = self.now();
        let wake_time = now + duration;
        assert!(wake_time >= now);

        SleepFut {
            current_time: self.current_time.clone(),
            time_events: self.time_events.clone(),
            wake_time,
        }
    }
}

pub struct SleepFut {
    current_time: Rc<AtomicU64>,
    time_events: Rc<RefCell<BinaryHeap<Reverse<TimeEvent>>>>,
    wake_time: Time,
}

impl Future for SleepFut {
    type Output = ();

    fn poll(self: Pin<&mut Self>, ctx: &mut Context<'_>) -> Poll<Self::Output> {
        let now = {
            let micros = self.current_time.load(Ordering::SeqCst);
            Time::from_micros(micros)
        };

        if now >= self.wake_time {
            Poll::Ready(())
        } else {
            let mut time_events = self.time_events.borrow_mut();
            time_events.push(Reverse(TimeEvent {
                wake_time: self.wake_time,
                waker: ctx.waker().clone(),
            }));

            Poll::Pending
        }
    }
}
