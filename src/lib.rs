extern crate libc;
extern crate time;
#[cfg(target_os = "macos")]
extern crate rand;

mod sys;
mod errno;
pub use sys::{
    Semaphore,
    SemaphoreGuard,
};

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use super::Semaphore;
    use std::sync::mpsc::channel;
    use std::thread;

    #[test]
    fn test_sem_acquire_post() {
        let s = Semaphore::new(1);
        s.wait();
        s.post();
        s.wait();
    }

    #[test]
    fn test_sem_basic() {
        let s = Semaphore::new(1);
        let _g = s.take();
    }

    #[test]
    fn test_sem_as_mutex() {
        let s = Arc::new(Semaphore::new(1));
        let s2 = s.clone();
        let _t = thread::spawn(move|| {
            let _g = s2.take();
        });
        let _g = s.take();
    }

    #[test]
    fn test_sem_as_cvar() {
        /* Child waits and parent signals */
        let (tx, rx) = channel();
        let s = Arc::new(Semaphore::new(0));
        let s2 = s.clone();
        let _t = thread::spawn(move|| {
            s2.wait();
            tx.send(()).unwrap();
        });
        s.post();
        let _ = rx.recv();

        /* Parent waits and child signals */
        let (tx, rx) = channel();
        let s = Arc::new(Semaphore::new(0));
        let s2 = s.clone();
        let _t = thread::spawn(move|| {
            s2.post();
            let _ = rx.recv();
        });
        s.wait();
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
            let _g = s2.take();
            let _ = rx2.recv();
            tx1.send(()).unwrap();
        });
        let _g = s.take();
        tx2.send(()).unwrap();
        rx1.recv().unwrap();
    }

    #[test]
    fn test_sem_runtime_friendly_blocking() {
        let s = Arc::new(Semaphore::new(1));
        let s2 = s.clone();
        let (tx, rx) = channel();
        {
            let _g = s.take();
            thread::spawn(move|| {
                tx.send(()).unwrap();
                drop(s2.take());
                tx.send(()).unwrap();
            });
            rx.recv().unwrap(); // wait for child to come alive
        }
        rx.recv().unwrap(); // wait for child to be done
    }
}
