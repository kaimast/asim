#![allow(incomplete_features)]
#![feature(inherent_associated_types)]

/// asim is a discrete event simulator for Rust
///
/// It implements an asynchronous runtime and provides utility function
/// to write such simulations in code that is very similar to the real thing.
use std::cell::RefCell;
use std::future::Future;

pub mod sync;

pub mod network;

pub mod time;

pub mod runtime;
pub use runtime::Runtime;

pub use asim_macros::test;

mod task;
pub use task::Task;
pub(crate) use task::{RcWrapper, TaskQueue};

thread_local! {
    /// The currently active runtime, if any
    static CONTEXT: RefCell<Option<runtime::Handle>> = const { RefCell::new(None) };
}

/// Spawn a new task in the current asim context
///
/// Note, this will panic if no asim context is active
pub fn spawn(future: impl Future<Output = ()> + 'static) {
    CONTEXT.with(|hdl| {
        hdl.borrow()
            .as_ref()
            .expect("Not in an asim context!")
            .spawn(future)
    })
}

pub fn get_runtime() -> runtime::Handle {
    CONTEXT.with(|hdl| {
        hdl.borrow()
            .as_ref()
            .expect("Not in an asim context!")
            .clone()
    })
}
