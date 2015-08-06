pub use self::os::{
    Semaphore,
    SemaphoreGuard,
};

#[cfg(not(target_os = "macos"))]
mod os {
    use std::cell::UnsafeCell;
    use std::mem;
    use std::io;
    use libc::{
        c_int,
        c_uint,
    };

    #[cfg(target_pointer_width = "64")]
    const SIZEOF_SEM_T: usize = 32;
    #[cfg(not(target_pointer_width = "64"))]
    const SIZEOF_SEM_T: usize = 16;

    extern {
        fn sem_init(sem: *mut sem_t, pshared: c_int, value: c_uint) -> c_int;
        fn sem_post(sem: *mut sem_t) -> c_int;
        fn sem_wait(sem: *mut sem_t) -> c_int;
        fn sem_destroy(sem: *mut sem_t) -> c_int;
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

        pub fn wait(&self) {
            loop {
                let res = unsafe {
                    sem_wait(self.inner.get())
                };
                if res == -1 {
                    match io::Error::last_os_error() {
                        ref e if e.kind() == io::ErrorKind::Interrupted => continue,
                        other => panic!("{}", other),
                    }
                } else {
                    break;
                }
            }
        }

        pub fn post(&self) {
            let res = unsafe {
                sem_post(self.inner.get())
            };
            debug_assert_eq!(res, 0);
        }

        pub fn access(&self) -> SemaphoreGuard {
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
        c_int,
        c_uint,
        c_char,
        mode_t,
        O_CREAT,
        O_EXCL,
        S_IRWXU,
    };

    const SIZEOF_SEM_T: usize = 4;
    const SEM_NAME_MAX: usize = 28; // No definitive value for this on OS X. Erring on the side of caution.
    const SEM_FAILED: *mut sem_t = 0 as *mut sem_t;

    extern {
        fn sem_open(name: *const c_char, oflag: c_int, mode: mode_t, value: c_uint) -> *mut sem_t;
        fn sem_post(sem: *mut sem_t) -> c_int;
        fn sem_wait(sem: *mut sem_t) -> c_int;
        fn sem_close(sem: *mut sem_t) -> c_int;
        fn sem_unlink(sem: *const c_char) -> c_int;
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

        pub fn wait(&self) {
            loop {
                let res = unsafe {
                    sem_wait(*self.inner.get())
                };
                if res == -1 {
                    match io::Error::last_os_error() {
                        ref e if e.kind() == io::ErrorKind::Interrupted => continue,
                        other => panic!("{}", other),
                    }
                } else {
                    break;
                }
            }
        }

        pub fn post(&self) {
            let res = unsafe {
                sem_post(*self.inner.get())
            };
            debug_assert_eq!(res, 0);
        }

        pub fn access(&self) -> SemaphoreGuard {
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
