use std::cell::RefCell;
use std::future::Future;
use std::pin::Pin;
use std::rc::Rc;
use std::sync::Arc;

use parking_lot::{Mutex, MutexGuard};

use futures::task::ArcWake;

pub(crate) type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + 'a>>;

pub(crate) type TaskQueue = Vec<Rc<Task>>;

pub(crate) struct RcWrapper(Rc<Task>);

// This is needed to appease the Send/Sync requirement for futures
// asim uses a single thread, so this is always safe
unsafe impl Send for RcWrapper {}
unsafe impl Sync for RcWrapper {}

impl From<Rc<Task>> for RcWrapper {
    fn from(task: Rc<Task>) -> Self {
        Self(task)
    }
}

impl ArcWake for RcWrapper {
    fn wake_by_ref(self_ptr: &Arc<Self>) {
        let inner = &self_ptr.0;
        inner.ready_tasks.borrow_mut().push(inner.clone());
    }
}

pub struct Task {
    future: Mutex<Option<BoxFuture<'static, ()>>>,
    ready_tasks: Rc<RefCell<TaskQueue>>,
}

impl Task {
    pub(crate) fn new(
        future: impl Future<Output = ()> + 'static,
        ready_tasks: Rc<RefCell<TaskQueue>>,
    ) -> Self {
        let future = Box::pin(future);

        Self {
            future: Mutex::new(Some(future)),
            ready_tasks,
        }
    }

    pub(crate) fn get_future(&self) -> MutexGuard<'_, Option<BoxFuture<'static, ()>>> {
        self.future.lock()
    }
}
