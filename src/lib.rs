extern crate libc;
extern crate time;
extern crate rand;

mod sys;
pub use sys::{
    Semaphore,
    SemaphoreGuard,
};

