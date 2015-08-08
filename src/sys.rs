pub use self::os::{
    Semaphore,
    SemaphoreGuard,
};

// Linux-specific futex semaphores.
#[cfg(target_os = "linux")]
mod os {
    use std::mem;
    use std::ptr;
    use std::sync::atomic::{
        Ordering,
        AtomicUsize,
    };

    use libc;
    use time::Duration;

    use errno::errno;

    #[cfg(target_pointer_width = "64")]
    const NWAITERS_SHIFT: usize = 32;
    #[cfg(target_pointer_width = "32")]
    const NWAITERS_SHIFT: usize = 16;

    const VALUE_MASK: usize = (!0) >> NWAITERS_SHIFT;

    // Value to add to semaphoroe to add one waiter.
    const ONE_WAITER: usize = (1 << NWAITERS_SHIFT);
    // Value to add to semaphore to subtract one waiter.
    const NEG_ONE_WAITER: usize = (!0 << NWAITERS_SHIFT);


    #[cfg(target_arch = "x86_64")]
    const SYS_FUTEX: libc::c_long = 202;
    #[cfg(target_arch = "x86")]
    const SYS_FUTEX: libc::c_long = 240;

    const FUTEX_WAIT: i32 = 0;
    const FUTEX_WAKE: i32 = 1;


    extern {
        fn syscall(number: libc::c_long, ...) -> libc::c_long;
    }

    fn futex_wake(uaddr: *mut u32, val: u32) -> Result<i32, i32> {
        let res = unsafe {
            syscall(SYS_FUTEX, uaddr, FUTEX_WAKE, val)
        };
        if res == -1 {
            Err(errno())
        } else {
            Ok(res as i32)
        }
    }

    fn futex_wait(uaddr: *mut u32, val: u32, timeout: *const libc::timespec) -> Result<i32, i32> {
        let res = unsafe {
            syscall(SYS_FUTEX, uaddr, FUTEX_WAIT, val, timeout)
        };
        if res == -1 {
            Err(errno())
        } else {
            Ok(res as i32)
        }
    }

    unsafe trait AsPointer<T> {
        unsafe fn as_ptr(&self) -> *mut T;
    }

    // This is almost definitely undefined behaviour, however this is the only method to get the
    // address of the underlying integer contained in the atomic wrapper since the field is private.
    //
    // This is ONLY to pass to the kernel for futex syscalls and should never, ever, ever be done under
    // normal circumstances.
    unsafe impl AsPointer<usize> for AtomicUsize {
        unsafe fn as_ptr(&self) -> *mut usize {
            mem::transmute(self)
        }
    }

    // Converts a `Duration` to a `timespec`.
    fn to_timespec(dur: Duration) -> libc::timespec {
        let sec = dur.num_seconds();
        // Safe to unwrap since there can't be more than one second left.
        let nsec = (dur - Duration::seconds(sec)).num_nanoseconds().unwrap();
        libc::timespec {
            tv_sec: sec as libc::time_t,
            tv_nsec: nsec as libc::c_long,
        }
    }

    pub struct Semaphore {
        data: AtomicUsize,
    }

    pub struct SemaphoreGuard<'a> {
        sem: &'a Semaphore,
    }

    impl Semaphore {
        pub fn new(value: usize) -> Semaphore {
            Semaphore {
                data: AtomicUsize::new(value),
            }
        }

        pub fn post(&self) {
            let d = self.data.load(Ordering::Relaxed);
            // Release, pending the acquire which will establish happens-before relation.
            self.data.fetch_add(1, Ordering::Release);

            // If there are any waiters, wake one.
            if (d >> NWAITERS_SHIFT) > 0 {
                futex_wake(self.value_ptr(), 1).unwrap();
            }
        }

        pub fn wait(&self) -> Result<(), Error> {
            self.wait_fast(false).or_else(|_| {
                self.wait_slow(ptr::null())
            })
        }

        pub fn try_wait(&self) -> Result<(), Error> {
            self.wait_fast(true)
        }

        pub fn wait_timeout(&self, timeout: Duration) -> Result<(), Error> {
            self.wait_fast(false).or_else(|_| {
                let ts = to_timespec(timeout);
                self.wait_slow(&ts)
            })
        }

        pub fn take(&self) -> Result<SemaphoreGuard, Error> {
            try!(self.wait());
            Ok(SemaphoreGuard {
                sem: self,
            })
        }

        fn value_ptr(&self) -> *mut u32 {
            #[cfg(any(target_endian = "little",
                      target_pointer_width = "32"))]
            const VALUE_OFFSET: isize = 0;
            #[cfg(all(target_endian = "big",
                      target_pointer_width = "64"))]
            const VALUE_OFFSET: isize = 1;

            unsafe {
                (self.data.as_ptr() as *mut u32).offset(VALUE_OFFSET)
            }
        }

        fn wait_fast(&self, definitive_result: bool) -> Result<(), Error> {
            let mut d = self.data.load(Ordering::Relaxed);
            loop {
                // Check if there is a token available.
                if (d & VALUE_MASK) == 0 {
                    // No token available. Need to call `wait_slow()` and block.
                    return Err(Error::WouldBlock);
                }
                // Grab the token and establish synchronizes-with between threads.
                let prev = self.data.compare_and_swap(d, d - 1, Ordering::Acquire);
                if prev == d {
                    // Swap was successful and we have taken a token.
                    return Ok(())
                } else {
                    // Swap was unsuccessful. Update variable and possibly loop.
                    d = prev;
                }
                if definitive_result {
                    continue;
                } else {
                    return Err(Error::WouldBlock);
                }
            }
        }

        fn wait_slow(&self, timeout: *const libc::timespec) -> Result<(), Error> {
            let mut d = self.data.fetch_add(ONE_WAITER, Ordering::Relaxed);

            // Wait for a token to become available.
            loop {
                // If there is no token avalable, sleep until there is.
                if (d & VALUE_MASK) == 0 {
                    let res = futex_wait(self.value_ptr(), 0, timeout);

                    // If `futex_wait` timed out, or was interrupted by a signal, return this error to
                    // the caller. Otherwise we retry.
                    if let Err(e) = res {
                        if e == libc::EINTR || e == libc::ETIMEDOUT {
                            // Deregister ourselves as a waiter.
                            self.data.fetch_add(NEG_ONE_WAITER, Ordering::Relaxed);
                            if e == libc::EINTR {
                                return Err(Error::Interrupted);
                            } else {
                                return Err(Error::TimedOut);
                            }
                        }
                    }

                    d = self.data.load(Ordering::Relaxed);
                } else {
                    // There is a token available, try to take the token and decrement the number of
                    // waiters. Return if we are successful, loop if not.
                    let prev = self.data.compare_and_swap(d, (d - 1) - ONE_WAITER, Ordering::Acquire);
                    if prev == d {
                        // Swap was successful and we have synchronizes-with relationship.
                        return Ok(())
                    } else {
                        // Swap was unsuccessful. Update variable and retry.
                        d = prev;
                    }
                }
            }
        }
    }

    impl<'a> Drop for SemaphoreGuard<'a> {
        fn drop(&mut self) {
            self.sem.post();
        }
    }

    #[derive(Debug, Clone)]
    pub enum Error {
        Interrupted,
        TimedOut,
        WouldBlock,
        Unknown,
    }
}



// POSIX semaphores.
#[cfg(not(any(target_os = "macos",
              target_os = "linux")))]
mod os {
    use std::cell::UnsafeCell;
    use std::mem;
    use std::io;

    use time::Duration;
    use libc::{
        self,
        c_int,
        c_uint,
    };

    use errno::errno;

    #[cfg(target_pointer_width = "64")]
    const SIZEOF_SEM_T: usize = 32;
    #[cfg(not(target_pointer_width = "64"))]
    const SIZEOF_SEM_T: usize = 16;

    extern {
        fn sem_init(sem: *mut sem_t, pshared: c_int, value: c_uint) -> c_int;
        fn sem_post(sem: *mut sem_t) -> c_int;
        fn sem_wait(sem: *mut sem_t) -> c_int;
        fn sem_trywait(sem: *mut sem_t) -> c_int;
        fn sem_timedwait(sem: *mut sem_t, timeout: *const libc::timespec) -> c_int;
        fn sem_destroy(sem: *mut sem_t) -> c_int;
    }

    // Converts a `Duration` to a `timespec`.
    fn to_timespec(dur: Duration) -> libc::timespec {
        let sec = dur.num_seconds();
        // Safe to unwrap since there can't be more than one second left.
        let nsec = (dur - Duration::seconds(sec)).num_nanoseconds().unwrap();
        libc::timespec {
            tv_sec: sec as libc::time_t,
            tv_nsec: nsec as libc::c_long,
        }
    }

    #[repr(C)]
    #[derive(Debug)]
    struct sem_t {
        __opaque: [u8; SIZEOF_SEM_T],
    }

    pub struct Semaphore {
        inner: UnsafeCell<sem_t>,
    }

    pub struct SemaphoreGuard<'a> {
        sem: &'a Semaphore,
    }

    impl Semaphore {
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

        pub fn wait(&self) -> Result<(), Error> {
            let res = unsafe {
                sem_wait(self.inner.get())
            };
            if res == -1 {
                match errno() {
                    libc::EINTR => Err(Error::Interrupted),
                    _ => Err(Error::Unknown),
                }
            } else {
                Ok(())
            }
        }

        pub fn try_wait(&self) -> Result<(), Error> {
            let res = unsafe {
                sem_trywait(self.inner.get())
            };
            if res == -1 {
                match errno() {
                    libc::EINTR => Err(Error::Interrupted),
                    libc::EWOULDBLOCK => Err(Error::WouldBlock),
                    _ => Err(Error::Unknown),
                }
            } else {
                Ok(())
            }
        }

        pub fn wait_timeout(&self, timeout: Duration) -> Result<(), Error> {
            let res = unsafe {
                let ts = to_timespec(timeout);
                sem_timedwait(self.inner.get(), &ts)
            };
            if res == -1 {
                match errno() {
                    libc::EINTR => Err(Error::Interrupted),
                    libc::ETIMEDOUT => Err(Error::TimedOut),
                    _ => Err(Error::Unknown),
                }
            } else {
                Ok(())
            }
        }

        pub fn post(&self) {
            let res = unsafe {
                sem_post(self.inner.get())
            };
            debug_assert_eq!(res, 0);
        }

        pub fn take(&self) -> SemaphoreGuard {
            self.wait();
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

    #[derive(Debug, Clone)]
    pub enum Error {
        Interrupted,
        TimedOut,
        WouldBlock,
        Unknown,
    }

    impl<'a> Drop for SemaphoreGuard<'a> {
        fn drop(&mut self) {
            self.sem.post();
        }
    }
}

#[cfg(target_os = "macos")]
mod os {
    use std::ffi::CString;
    use std::cell::UnsafeCell;
    use std::io;

    use rand::{
        thread_rng,
        Rng,
    };
    use libc::{
        self,
        c_int,
        c_uint,
        c_char,
        mode_t,
        O_CREAT,
        O_EXCL,
        S_IRWXU,
    };

    use errno::errno;

    const SEM_NAME_MAX: usize = 28; // No definitive value for this on OS X. Erring on the side of caution.
    const SEM_FAILED: *mut sem_t = 0 as *mut sem_t;

    extern {
        fn sem_open(name: *const c_char, oflag: c_int, mode: mode_t, value: c_uint) -> *mut sem_t;
        fn sem_post(sem: *mut sem_t) -> c_int;
        fn sem_wait(sem: *mut sem_t) -> c_int;
        fn sem_trywait(sem: *mut sem_t) -> c_int;
        fn sem_timedwait(sem: *mut sem_t, timeout: *const libc::timespec) -> c_int;
        fn sem_close(sem: *mut sem_t) -> c_int;
        fn sem_unlink(sem: *const c_char) -> c_int;
    }

    // Converts a `Duration` to a `timespec`.
    fn to_timespec(dur: Duration) -> libc::timespec {
        let sec = dur.num_seconds();
        // Safe to unwrap since there can't be more than one second left.
        let nsec = (dur - Duration::seconds(sec)).num_nanoseconds().unwrap();
        libc::timespec {
            tv_sec: sec as libc::time_t,
            tv_nsec: nsec as libc::c_long,
        }
    }

    #[repr(C)]
    #[derive(Debug)]
    pub struct sem_t {
        __opaque: c_int,
    }

    pub struct Semaphore {
        inner: UnsafeCell<*mut sem_t>,
        name: CString,
    }

    pub struct SemaphoreGuard<'a> {
        sem: &'a Semaphore,
    }

    impl Semaphore {
        pub fn new(value: u32) -> Semaphore {
            let name: String = thread_rng().gen_ascii_chars().take(SEM_NAME_MAX).collect();
            let c_name = CString::new(name).unwrap(); // Rng does not emit 0 bytes.

            let sem: *mut sem_t = unsafe {
                sem_open(c_name.as_ptr(), O_CREAT | O_EXCL, S_IRWXU, value as c_uint)
            };
            debug_assert!(sem != SEM_FAILED);

            Semaphore {
                inner: UnsafeCell::new(sem),
                name: c_name,
            }
        }

        pub fn wait(&self) -> Result<(), Error> {
            let res = unsafe {
                sem_wait(self.inner.get())
            };
            if res == -1 {
                match errno() {
                    libc::EINTR => Err(Error::Interrupted),
                    _ => Err(Error::Unknown),
                }
            } else {
                Ok(())
            }
        }

        pub fn try_wait(&self) -> Result<(), Error> {
            let res = unsafe {
                sem_trywait(self.inner.get())
            };
            if res == -1 {
                match errno() {
                    libc::EINTR => Err(Error::Interrupted),
                    libc::EWOULDBLOCK => Err(Error::WouldBlock),
                    _ => Err(Error::Unknown),
                }
            } else {
                Ok(())
            }
        }

        pub fn wait_timeout(&self, timeout: Duration) -> Result<(), Error> {
            let res = unsafe {
                let ts = to_timespec(timeout);
                sem_timedwait(self.inner.get(), &ts)
            };
            if res == -1 {
                match errno() {
                    libc::EINTR => Err(Error::Interrupted),
                    libc::ETIMEDOUT => Err(Error::TimedOut),
                    _ => Err(Error::Unknown),
                }
            } else {
                Ok(())
            }
        }


        pub fn post(&self) {
            let res = unsafe {
                sem_post(*self.inner.get())
            };
            debug_assert_eq!(res, 0);
        }

        pub fn take(&self) -> SemaphoreGuard {
            self.wait();
            SemaphoreGuard { sem: self }
        }
    }

    unsafe impl Send for Semaphore {}
    unsafe impl Sync for Semaphore {}

    impl Drop for Semaphore {
        fn drop(&mut self) {
            let res = unsafe {
                sem_close(*self.inner.get())
            };
            debug_assert_eq!(res, 0);
            let res = unsafe {
                sem_unlink(self.name.as_ptr())
            };
            debug_assert_eq!(res, 0);
        }
    }

    impl<'a> Drop for SemaphoreGuard<'a> {
        fn drop(&mut self) {
            self.sem.post();
        }
    }
}
