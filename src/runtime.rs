use std::cell::RefCell;
use std::future::Future;
use std::rc::Rc;
use std::sync::Arc;
use std::task::Context;

use futures::task::waker_ref;

use crate::time::Timer;
use crate::{RcWrapper, Task, TaskQueue, CONTEXT};

/// An event queue servers as an executor for the async tasks simulating the timed events
pub struct Runtime {
    ready_tasks: Rc<RefCell<TaskQueue>>,
    timer: Rc<Timer>,
}

impl Default for Runtime {
    fn default() -> Self {
        let ready_tasks = Default::default();
        Self {
            ready_tasks,
            timer: Rc::new(Timer::new()),
        }
    }
}

impl Runtime {
    /// Run all ready tasks
    /// Will return true if any task ran
    pub fn execute_tasks(&self) -> bool {
        let ready_tasks = {
            let mut tasks = self.ready_tasks.borrow_mut();
            std::mem::take(&mut *tasks)
        };

        if ready_tasks.is_empty() {
            return false;
        } else {
            log::trace!("Found {} tasks that are ready", ready_tasks.len());
        }

        // Set the asim context before we run
        CONTEXT.with(|hdl| {
            let mut context = hdl.borrow_mut();
            if context.is_some() {
                panic!("We are already in an asim context!");
            }
            *context = Some(self.handle());
        });

        for task in ready_tasks.into_iter() {
            let mut fut_lock = task.get_future();

            if let Some(mut future) = fut_lock.take() {
                let wrapper: RcWrapper = task.clone().into();
                let wrapper = Arc::new(wrapper);
                let waker = waker_ref(&wrapper);
                let context = &mut Context::from_waker(&waker);

                if future.as_mut().poll(context).is_pending() {
                    *fut_lock = Some(future);
                }
            }
        }

        CONTEXT.with(|hdl| {
            *hdl.borrow_mut() = None;
        });

        true
    }

    pub fn spawn(&self, future: impl Future<Output = ()> + 'static) {
        let task = Rc::new(Task::new(future, self.ready_tasks.clone()));
        self.ready_tasks.borrow_mut().push(task);
    }

    /// Drops all queued events
    pub fn stop(&self) {
        self.ready_tasks.borrow_mut().clear();
    }

    /// Creates a handle to this runtime
    /// that can be passed around
    pub fn handle(&self) -> Handle {
        Handle {
            ready_tasks: self.ready_tasks.clone(),
            timer: self.timer.clone(),
        }
    }

    pub fn get_timer(&self) -> &Timer {
        &self.timer
    }
}

#[derive(Clone)]
pub struct Handle {
    ready_tasks: Rc<RefCell<TaskQueue>>,
    timer: Rc<Timer>,
}

impl Handle {
    pub fn spawn(&self, future: impl Future<Output = ()> + 'static) {
        let task = Rc::new(Task::new(future, self.ready_tasks.clone()));
        self.ready_tasks.borrow_mut().push(task);
    }

    /// Drops all queued events
    pub fn stop(&self) {
        self.ready_tasks.borrow_mut().clear();
    }

    pub fn get_timer(&self) -> &Timer {
        &self.timer
    }
}
