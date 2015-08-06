#![feature(test)]
#![feature(semaphore)]

extern crate test;
extern crate sema;

use std::sync::Semaphore as StdSemaphore;

use test::Bencher;
use sema::Semaphore;

#[bench]
fn sema_take(b: &mut Bencher) {
    let sema = Semaphore::new(1);
    b.iter(|| {
        let _guard = sema.take();
    });
}

#[bench]
fn std_take(b: &mut Bencher) {
    let sem = StdSemaphore::new(1);
    b.iter(|| {
        let _guard = sem.access();
    });
}

#[bench]
fn sema_wait_post(b: &mut Bencher) {
    let sema = Semaphore::new(1);
    b.iter(|| {
        sema.wait();
        sema.post();
    });
}

#[bench]
fn std_wait_post(b: &mut Bencher) {
    let sem = StdSemaphore::new(1);
    b.iter(|| {
        sem.acquire();
        sem.release();
    });
}

#[bench]
fn sema_new(b: &mut Bencher) {
    b.iter(|| {
        let _sema = Semaphore::new(1);
    });
}

#[bench]
fn std_new(b: &mut Bencher) {
    b.iter(|| {
        let _sem = StdSemaphore::new(1);
    });
}

