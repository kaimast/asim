/// Primitives that are *not* safe to use while await'ing, but more lightweight
use std::cell::{RefCell, RefMut};
use std::future::Future;
use std::ops::{Deref, DerefMut};
use std::pin::Pin;
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::task::{Context, Poll, Waker};

use crate::time::{SleepFut, Duration};

type CondWaiters = Vec<(Rc<AtomicBool>, Waker)>;

#[derive(Default)]
pub struct SyncCondvar {
    waiters: Rc<RefCell<CondWaiters>>,
}

pub struct SyncCondWait<'a, T> {
    mutex: &'a SyncMutex<T>,
    woken: Rc<AtomicBool>,
    waiters: Rc<RefCell<CondWaiters>>,
}

pub struct SyncCondTimeoutWait<'a, T> {
    mutex: &'a SyncMutex<T>,
    woken: Rc<AtomicBool>,
    sleep_fut: SleepFut,
    waiters: Rc<RefCell<CondWaiters>>,
}

/// A mutex that is just a wrapper around RefCell
/// Do not hold this lock while calling await
pub struct SyncMutex<T> {
    inner: RefCell<T>,
}

pub struct SyncLockGuard<'a, T> {
    data: RefMut<'a, T>,
    mutex: &'a SyncMutex<T>,
}

impl<'a, T> SyncLockGuard<'a, T> {
    fn into_mutex(self) -> &'a SyncMutex<T> {
        self.mutex
    }
}

impl<T> SyncMutex<T> {
    pub fn new(data: T) -> Self {
        Self {
            inner: RefCell::new(data),
        }
    }

    pub fn lock(&self) -> SyncLockGuard<'_, T> {
        let data = self.inner.borrow_mut();
        SyncLockGuard { data, mutex: self }
    }
}

impl<T: Default> Default for SyncMutex<T> {
    fn default() -> Self {
        Self::new(T::default())
    }
}

impl<T> Deref for SyncLockGuard<'_, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.data
    }
}

impl<T> DerefMut for SyncLockGuard<'_, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.data
    }
}

impl<'a, T> Future for SyncCondWait<'a, T> {
    type Output = SyncLockGuard<'a, T>;

    fn poll(self: Pin<&mut Self>, ctx: &mut Context<'_>) -> Poll<Self::Output> {
        if self.woken.load(Ordering::SeqCst) {
            Poll::Ready(self.mutex.lock())
        } else {
            let mut waiters = self.waiters.borrow_mut();
            waiters.push((self.woken.clone(), ctx.waker().clone()));
            Poll::Pending
        }
    }
}

impl<'a, T> Future for SyncCondTimeoutWait<'a, T> {
    type Output = SyncLockGuard<'a, T>;

    fn poll(mut self: Pin<&mut Self>, ctx: &mut Context<'_>) -> Poll<Self::Output> {
        if self.woken.load(Ordering::SeqCst) {
            Poll::Ready(self.mutex.lock())
        } else if SleepFut::poll(Pin::new(&mut self.sleep_fut), ctx) == Poll::Ready(()) {
            log::trace!("Condvar::wait timed out");
            Poll::Ready(self.mutex.lock())
        } else {
            let mut waiters = self.waiters.borrow_mut();
            waiters.push((self.woken.clone(), ctx.waker().clone()));
            Poll::Pending
        }
    }
}

impl SyncCondvar {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn wait<'a, T>(&self, lock: SyncLockGuard<'a, T>) -> SyncCondWait<'a, T> {
        let mutex = lock.into_mutex();

        SyncCondWait {
            mutex,
            waiters: self.waiters.clone(),
            woken: Rc::new(AtomicBool::new(false)),
        }
    }

    pub fn wait_with_timeout<'a, T>(
        &self,
        lock: SyncLockGuard<'a, T>,
        timeout: Duration,
    ) -> SyncCondTimeoutWait<'a, T> {
        assert!(!timeout.is_zero());
        let mutex = lock.into_mutex();

        SyncCondTimeoutWait {
            mutex,
            sleep_fut: crate::time::sleep(timeout),
            waiters: self.waiters.clone(),
            woken: Rc::new(AtomicBool::new(false)),
        }
    }

    pub fn notify_one(&self) {
        let mut waiters = self.waiters.borrow_mut();
        let mut old_waiters = vec![];
        std::mem::swap(&mut *waiters, &mut old_waiters);

        let mut iter = old_waiters.into_iter();

        // Find one waker that has not been woken yet
        for (is_woken, waker) in iter.by_ref() {
            if !is_woken.load(Ordering::SeqCst) {
                is_woken.store(true, Ordering::SeqCst);
                waker.wake();
                break;
            }
        }

        // Put the rest back on the waiter list
        for e in iter {
            waiters.push(e);
        }
    }

    pub fn notify_all(&self) {
        let mut waiters = self.waiters.borrow_mut();

        for (is_woken, waker) in waiters.drain(..) {
            if !is_woken.load(Ordering::SeqCst) {
                is_woken.store(true, Ordering::SeqCst);
                waker.wake();
            }
        }
    }
}
