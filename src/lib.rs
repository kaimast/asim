use std::cell::RefCell;
use std::future::Future;
use std::pin::Pin;
use std::rc::Rc;
use std::sync::Arc;
use std::task::Context;

use parking_lot::Mutex;

use futures::task::{waker_ref, ArcWake};

pub mod sync;

pub mod network;

pub mod timer;
pub use timer::Timer;

pub mod time;

type TaskQueue = Vec<Rc<Task>>;

type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + 'a>>;

struct Task {
    future: Mutex<Option<BoxFuture<'static, ()>>>,
    ready_tasks: Rc<RefCell<TaskQueue>>,
}

struct RcWrapper(Rc<Task>);
unsafe impl Send for RcWrapper {}
unsafe impl Sync for RcWrapper {}

impl ArcWake for RcWrapper {
    fn wake_by_ref(self_ptr: &Arc<Self>) {
        let inner = &self_ptr.0;
        inner.ready_tasks.borrow_mut().push(inner.clone());
    }
}

thread_local! {
    /// The event queue of the active task runner, if any
    static TASK_RUNNER: RefCell<Option<Rc<RefCell<TaskQueue>>>> = RefCell::new(None);
}

/// An event queue servers as an executor for the async tasks simulating the timed events
pub struct TaskRunner {
    ready_tasks: Rc<RefCell<TaskQueue>>,
}

impl Default for TaskRunner {
    fn default() -> Self {
        let ready_tasks = Default::default();
        Self { ready_tasks }
    }
}

impl TaskRunner {
    /// Run all ready tasks
    /// Will return true if any task ran
    pub fn execute_tasks(&self) -> bool {
        let mut ready_tasks = {
            let mut tasks = self.ready_tasks.borrow_mut();
            std::mem::take(&mut *tasks)
        };

        if ready_tasks.is_empty() {
            return false;
        } else {
            log::trace!("Found {} tasks that are ready", ready_tasks.len());
        }

        {
            let task_queue = self.ready_tasks.clone();
            TASK_RUNNER.with(|r| {
                r.borrow_mut().replace(task_queue);
            });
        }

        for task in ready_tasks.drain(..) {
            let mut fut_lock = task.future.lock();

            if let Some(mut future) = fut_lock.take() {
                let wrapper = Arc::new(RcWrapper(task.clone()));
                let waker = waker_ref(&wrapper);
                let context = &mut Context::from_waker(&waker);

                if future.as_mut().poll(context).is_pending() {
                    *fut_lock = Some(future);
                }
            }
        }

        TASK_RUNNER.with(|r| {
            *r.borrow_mut() = None;
        });

        true
    }

    pub fn spawn(&self, future: impl Future<Output = ()> + 'static) {
        let future = Box::pin(future);
        let task = Rc::new(Task {
            future: Mutex::new(Some(future)),
            ready_tasks: self.ready_tasks.clone(),
        });

        self.ready_tasks.borrow_mut().push(task);
    }

    /// Drops all queued events
    pub fn stop(&self) {
        self.ready_tasks.borrow_mut().clear();
    }
}

/// Spawn a new task in the current asim context
///
/// Note, this will panic if no asim context is active
pub fn spawn(future: impl Future<Output = ()> + 'static) {
    TASK_RUNNER.with(|r| {
        let hdl = r.borrow();
        let ready_tasks = hdl.as_ref().expect("Not in a asim context!");

        let future = Box::pin(future);
        let task = Rc::new(Task {
            future: Mutex::new(Some(future)),
            ready_tasks: ready_tasks.clone(),
        });

        ready_tasks.borrow_mut().push(task);
    });
}
