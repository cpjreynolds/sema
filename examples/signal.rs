extern crate libc;
extern crate nix;
#[macro_use]
extern crate lazy_static;
extern crate sema;

use sema::Semaphore;
use nix::sys::signal::{
    sigaction,
    SigAction,
    SockFlag,
    SigSet,
};
use nix::sys::signal::signal::SIGWINCH;

use std::sync::atomic::{
    AtomicUsize,
    ATOMIC_USIZE_INIT,
    Ordering,
};
use std::thread;

lazy_static! {
    static ref SIGNAL_SEMA: Semaphore = {
        Semaphore::new(0)
    };
}

static SIGNAL_MASK: AtomicUsize = ATOMIC_USIZE_INIT;

fn main() {
    let sa = SigAction::new(sighandler, SockFlag::empty(), SigSet::empty());
    unsafe { sigaction(SIGWINCH, &sa).unwrap() };
    let t = thread::spawn(move || {
        loop {
            SIGNAL_SEMA.wait();
            let last = SIGNAL_MASK.fetch_and(!(1<<SIGWINCH), Ordering::SeqCst);
            if last & (1<<SIGWINCH) != 0 {
                println!("Caught SIGWINCH");
            }
        }
    });
    t.join().unwrap();
}

extern fn sighandler(signum: i32) {
    SIGNAL_MASK.fetch_or((1<<signum), Ordering::SeqCst);
    SIGNAL_SEMA.post();
}

