extern crate libc;

use std::cell::UnsafeCell;
use std::io::{
    Error,
    ErrorKind,
};
use std::mem;

use libc::{
//    c_char,
    c_int,
    c_uint,
//    mode_t,
};

use self::os::sem_t;

//const SEM_FAILED: *mut sem_t = 0 as *mut sem_t;
// NAME_MAX - 4.
//const SEM_NAME_MAX: usize = 251;

extern {
    fn sem_init(sem: *mut sem_t, pshared: c_int, value: c_uint) -> c_int;
    //fn sem_open(name: *const c_char, oflag: c_int, mode: mode_t, value: c_uint) -> *mut sem_t;
    fn sem_post(sem: *mut sem_t) -> c_int;
    fn sem_wait(sem: *mut sem_t) -> c_int;
    fn sem_destroy(sem: *mut sem_t) -> c_int;
    //fn sem_close(sem: *mut sem_t) -> c_int;
    //fn sem_unlink(sem: *const c_char) -> c_int;
}

#[cfg(not(target_os = "macos"))]
mod os {
    #[cfg(target_pointer_width = "64")]
    const SIZEOF_SEM_T: usize = 32;
    #[cfg(not(target_pointer_width = "64"))]
    const SIZEOF_SEM_T: usize = 16;

    #[repr(C)]
    #[derive(Debug)]
    pub struct sem_t {
        __opaque: [u8; SIZEOF_SEM_T],
    }
}

#[cfg(target_os = "macos")]
mod os {
    use libc::c_int;

    #[repr(C)]
    #[derive(Debug)]
    pub struct sem_t {
        __opaque: c_int,
    }
}

/// A counting, blocking, semaphore.
///
/// Semaphores are a form of atomic counter where access is only granted if the
/// counter is a positive value. Each acquisition will block the calling thread
/// until the counter is positive, and each release will increment the counter
/// and unblock any threads if necessary.
///
/// # Examples
///
/// ```
/// use sema::Semaphore;
///
/// // Create a semaphore that represents 5 resources
/// let sem = Semaphore::new(5);
///
/// // Acquire one of the resources
/// sem.acquire();
///
/// // Acquire one of the resources for a limited period of time
/// {
///     let _guard = sem.access();
///     // ...
/// } // resources is released here
///
/// // Release our initially acquired resource
/// sem.release();
/// ```
pub struct Semaphore {
    inner: UnsafeCell<sem_t>,
}

/// An RAII guard which will release a resource acquired from a semaphore when
/// dropped.
pub struct SemaphoreGuard<'a> {
    sem: &'a Semaphore,
}

impl Semaphore {
    /// Creates a new semaphore with the initial count specified.
    ///
    /// The count specified can be thought of as a number of resources, and a
    /// call to `acquire` or `access` will block until at least one resource is
    /// available.
    ///
    /// **Note:** Unlike the standard library, this semaphore may not be initialized with a
    /// negative count.
    pub fn new(value: u32) -> Semaphore {
        let mut sem: sem_t = unsafe {
            mem::uninitialized()
        };
        let res = unsafe {
            sem_init(&mut sem, 0, value as c_uint)
        };
        debug_assert_eq!(res, 0);

        Semaphore {
            inner: UnsafeCell::new(sem),
        }
    }

    /// Acquires a resource of this semaphore, blocking the current thread until
    /// it can do so.
    ///
    /// This method will block until the internal count of the semaphore is at
    /// least 1.
    pub fn acquire(&self) {
        loop {
            let res = unsafe {
                sem_wait(self.inner.get())
            };
            if res == -1 {
                match Error::last_os_error() {
                    ref e if e.kind() == ErrorKind::Interrupted => continue,
                    other => panic!("{}", other),
                }
            } else {
                break;
            }
        }
    }

    /// Release a resource from this semaphore.
    ///
    /// This will increment the number of resources in this semaphore by 1 and
    /// will notify any pending waiters in `acquire` or `access` if necessary.
    /// 
    /// **Note:** This function is defined by POSIX.1-2001 to be async-signal-safe, and may be
    /// safely called within a signal handler.
    pub fn release(&self) {
        let res = unsafe {
            sem_post(self.inner.get())
        };
        debug_assert_eq!(res, 0);
    }

    /// Acquires a resource of this semaphore, returning an RAII guard to
    /// release the semaphore when dropped.
    ///
    /// This function is semantically equivalent to an `acquire` followed by a
    /// `release` when the guard returned is dropped.
    pub fn access(&self) -> SemaphoreGuard {
        self.acquire();
        SemaphoreGuard { sem: self }
    }
}

unsafe impl Send for Semaphore {}
unsafe impl Sync for Semaphore {}

impl Drop for Semaphore {
    fn drop(&mut self) {
        let res = unsafe {
            sem_destroy(self.inner.get())
        };
        debug_assert_eq!(res, 0);
    }
}

impl<'a> Drop for SemaphoreGuard<'a> {
    fn drop(&mut self) {
        self.sem.release();
    }
}

// These tests are taken from the Rust standard library semaphore implementation. Since we
// implement the same interface, (and use the same docs), it makes sense use the same tests as well.
#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use super::Semaphore;
    use std::sync::mpsc::channel;
    use std::thread;

    #[test]
    fn test_sem_acquire_release() {
        let s = Semaphore::new(1);
        s.acquire();
        s.release();
        s.acquire();
    }

    #[test]
    fn test_sem_basic() {
        let s = Semaphore::new(1);
        let _g = s.access();
    }

    #[test]
    fn test_sem_as_mutex() {
        let s = Arc::new(Semaphore::new(1));
        let s2 = s.clone();
        let _t = thread::spawn(move|| {
            let _g = s2.access();
        });
        let _g = s.access();
    }

    #[test]
    fn test_sem_as_cvar() {
        /* Child waits and parent signals */
        let (tx, rx) = channel();
        let s = Arc::new(Semaphore::new(0));
        let s2 = s.clone();
        let _t = thread::spawn(move|| {
            s2.acquire();
            tx.send(()).unwrap();
        });
        s.release();
        let _ = rx.recv();

        /* Parent waits and child signals */
        let (tx, rx) = channel();
        let s = Arc::new(Semaphore::new(0));
        let s2 = s.clone();
        let _t = thread::spawn(move|| {
            s2.release();
            let _ = rx.recv();
        });
        s.acquire();
        tx.send(()).unwrap();
    }

    #[test]
    fn test_sem_multi_resource() {
        // Parent and child both get in the critical section at the same
        // time, and shake hands.
        let s = Arc::new(Semaphore::new(2));
        let s2 = s.clone();
        let (tx1, rx1) = channel();
        let (tx2, rx2) = channel();
        let _t = thread::spawn(move|| {
            let _g = s2.access();
            let _ = rx2.recv();
            tx1.send(()).unwrap();
        });
        let _g = s.access();
        tx2.send(()).unwrap();
        rx1.recv().unwrap();
    }

    #[test]
    fn test_sem_runtime_friendly_blocking() {
        let s = Arc::new(Semaphore::new(1));
        let s2 = s.clone();
        let (tx, rx) = channel();
        {
            let _g = s.access();
            thread::spawn(move|| {
                tx.send(()).unwrap();
                drop(s2.access());
                tx.send(()).unwrap();
            });
            rx.recv().unwrap(); // wait for child to come alive
        }
        rx.recv().unwrap(); // wait for child to be done
    }
}
