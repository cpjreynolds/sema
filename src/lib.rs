extern crate libc;
extern crate time;
extern crate rand;

mod sys;
mod errno;
pub use sys::{
    Semaphore,
    SemaphoreGuard,
};

