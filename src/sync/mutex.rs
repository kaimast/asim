use std::cell::{RefCell, RefMut};
use std::future::Future;
use std::ops::{Deref, DerefMut};
use std::pin::Pin;
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::task::{Context, Poll, Waker};

pub struct MutexInner {
    is_locked: bool,
    next_waiter_id: u32,
}

type Waiters = Vec<(u32, Waker)>;

/// A Mutex that has an async lock function
/// Useful if you want to hold a lock while waiting for some other task to complete
pub struct Mutex<T> {
    data: RefCell<T>,
    inner: RefCell<MutexInner>,
    waiters: RefCell<Waiters>,
}

type CondWaiters = Vec<(Rc<AtomicBool>, Waker)>;

pub struct Condvar {
    waiters: Rc<RefCell<CondWaiters>>,
}

pub struct CondWait<'a, T> {
    mutex: &'a Mutex<T>,
    lock_future: RefCell<Option<LockFuture<'a, T>>>,
    woken: Rc<AtomicBool>,
    waiters: Rc<RefCell<CondWaiters>>,
}

pub struct LockFuture<'a, T> {
    identifier: u32,
    mutex: &'a Mutex<T>,
}

pub struct LockGuard<'a, T> {
    data: RefMut<'a, T>,
    mutex: &'a Mutex<T>,
}

impl<'a, T> LockGuard<'a, T> {
    fn into_mutex(self) -> &'a Mutex<T> {
        self.mutex
    }
}

impl<T> Drop for LockGuard<'_, T> {
    fn drop(&mut self) {
        let mut inner = self.mutex.inner.borrow_mut();
        inner.is_locked = false;

        let waiters = self.mutex.waiters.borrow_mut();
        if !waiters.is_empty() {
            waiters[0].1.wake_by_ref();
        }
    }
}

impl<T> Mutex<T> {
    pub fn new(data: T) -> Self {
        Self {
            data: RefCell::new(data),
            inner: RefCell::new(MutexInner {
                is_locked: false,
                next_waiter_id: 0,
            }),
            waiters: RefCell::new(vec![]),
        }
    }

    pub fn lock(&self) -> LockFuture<T> {
        let mut inner = self.inner.borrow_mut();
        let identifier = inner.next_waiter_id;
        inner.next_waiter_id += 1;

        LockFuture {
            identifier,
            mutex: self,
        }
    }
}

impl<T: Default> Default for Mutex<T> {
    fn default() -> Self {
        Self::new(T::default())
    }
}

impl<T> Deref for LockGuard<'_, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.data
    }
}

impl<T> DerefMut for LockGuard<'_, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.data
    }
}

impl<'a, T> Future for LockFuture<'a, T> {
    type Output = LockGuard<'a, T>;

    fn poll(self: Pin<&mut Self>, ctx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut inner = self.mutex.inner.borrow_mut();

        if !inner.is_locked {
            inner.is_locked = true;
            let mut waiters = self.mutex.waiters.borrow_mut();
            for (idx, (id, _)) in waiters.iter().enumerate() {
                if *id == self.identifier {
                    waiters.remove(idx);
                    break;
                }
            }

            let data = self.mutex.data.borrow_mut();

            Poll::Ready(LockGuard {
                data,
                mutex: self.mutex,
            })
        } else {
            let mut waiters = self.mutex.waiters.borrow_mut();
            let mut found = false;

            for (id, _) in waiters.iter() {
                if *id == self.identifier {
                    found = true;
                    break;
                }
            }

            if !found {
                waiters.push((self.identifier, ctx.waker().clone()));
            }

            Poll::Pending
        }
    }
}

impl<'a, T> Future for CondWait<'a, T> {
    type Output = LockGuard<'a, T>;

    fn poll(self: Pin<&mut Self>, ctx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut lock_future = self.lock_future.borrow_mut();

        if let Some(fut) = &mut *lock_future {
            LockFuture::poll(Pin::new(&mut *fut), ctx)
        } else if self.woken.load(Ordering::SeqCst) {
            let mut fut = self.mutex.lock();

            if let Poll::Ready(out) = LockFuture::poll(Pin::new(&mut fut), ctx) {
                Poll::Ready(out)
            } else {
                *lock_future = Some(fut);

                Poll::Pending
            }
        } else {
            let mut waiters = self.waiters.borrow_mut();
            waiters.push((self.woken.clone(), ctx.waker().clone()));

            Poll::Pending
        }
    }
}

impl Condvar {
    pub fn new() -> Self {
        Self {
            waiters: Rc::new(RefCell::new(vec![])),
        }
    }

    pub fn wait<'a, T>(&self, lock: LockGuard<'a, T>) -> CondWait<'a, T> {
        let mutex = lock.into_mutex();

        CondWait {
            mutex,
            lock_future: RefCell::new(None),
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

impl Default for Condvar {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use std::future::Future;
    use std::pin::Pin;
    use std::sync::Arc;
    use std::task::{Context, Poll};

    use super::{CondWait, Condvar, LockFuture, Mutex};

    use futures::task::{waker_ref, ArcWake};

    struct DummyWaker {}

    impl ArcWake for DummyWaker {
        fn wake_by_ref(_self_ptr: &Arc<Self>) {}
    }

    #[test]
    fn lock_unlock_mutex() {
        let mutex = Mutex::new(());

        {
            let waker = Arc::new(DummyWaker {});
            let waker = waker_ref(&waker);
            let context = &mut Context::from_waker(&waker);

            let mut lock_fut = mutex.lock();
            let res = LockFuture::poll(Pin::new(&mut lock_fut), context);
            assert!(matches!(res, Poll::Ready(_)));

            let waker = Arc::new(DummyWaker {});
            let waker = waker_ref(&waker);
            let context = &mut Context::from_waker(&waker);

            // This lock should fail as the other one is still held
            let mut lock_fut = mutex.lock();
            let res = LockFuture::poll(Pin::new(&mut lock_fut), context);
            assert!(matches!(res, Poll::Pending));
        }

        // Dropping unlocks it, so we should be able to lock again
        {
            let waker = Arc::new(DummyWaker {});
            let waker = waker_ref(&waker);
            let context = &mut Context::from_waker(&waker);

            let mut lock_fut = mutex.lock();
            let res = LockFuture::poll(Pin::new(&mut lock_fut), context);
            assert!(matches!(res, Poll::Ready(_)));
        }
    }

    #[test]
    fn condvar_notify() {
        let mutex = Mutex::new(());
        let condvar = Condvar::new();

        // Need to grab lock before calling wait
        let waker = Arc::new(DummyWaker {});
        let waker = waker_ref(&waker);
        let context = &mut Context::from_waker(&waker);

        let mut lock_fut = mutex.lock();
        let lock_guard =
            if let Poll::Ready(guard) = LockFuture::poll(Pin::new(&mut lock_fut), context) {
                guard
            } else {
                panic!("Lock returned pending");
            };

        let mut wait_fut = condvar.wait(lock_guard);

        // We haven't called notify yet so this should return pending
        let res = CondWait::poll(Pin::new(&mut wait_fut), context);
        assert!(matches!(res, Poll::Pending));

        condvar.notify_one();

        // Now it should succeed
        let res = CondWait::poll(Pin::new(&mut wait_fut), context);
        assert!(matches!(res, Poll::Ready(_)));
    }
}
