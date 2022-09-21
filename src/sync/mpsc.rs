use parking_lot::Mutex;

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll, Waker};

#[derive(Default)]
struct Inner<T> {
    messages: Vec<T>,
    waker: Option<Waker>,
}

pub struct Sender<T> {
    inner: Arc<Mutex<Inner<T>>>,
}

impl<T> Sender<T> {
    pub fn send(&self, msg: T) {
        let mut inner = self.inner.lock();
        inner.messages.push(msg);

        if let Some(waker) = inner.waker.take() {
            waker.wake();
        }
    }
}

pub struct Receiver<T> {
    inner: Arc<Mutex<Inner<T>>>,
}

impl<T> Receiver<T> {
    #[must_use]
    pub fn recv(&self) -> GetFut<T> {
        GetFut {
            inner: self.inner.clone(),
        }
    }
}

pub struct GetFut<T> {
    inner: Arc<Mutex<Inner<T>>>,
}

impl<T> Future for GetFut<T> {
    type Output = Vec<T>;

    fn poll(self: Pin<&mut Self>, ctx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut lock = self.inner.lock();

        if lock.messages.is_empty() {
            lock.waker = Some(ctx.waker().clone());
            Poll::Pending
        } else {
            let mut messages = vec![];
            std::mem::swap(&mut messages, &mut lock.messages);
            Poll::Ready(messages)
        }
    }
}

pub fn channel<T>() -> (Sender<T>, Receiver<T>) {
    let inner = Arc::new(Mutex::new(Inner {
        messages: vec![],
        waker: None,
    }));

    (
        Sender {
            inner: inner.clone(),
        },
        Receiver { inner },
    )
}
