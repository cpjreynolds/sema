#![feature(test)]

extern crate test;
extern crate sema;

use test::Bencher;
use sema::Semaphore;

#[bench]
fn sema_access(b: &mut Bencher) {
    let sema = Semaphore::new(1);
    b.iter(|| {
        let _guard = sema.access();
    });
}

#[bench]
fn sema_acq_rel(b: &mut Bencher) {
    let sema = Semaphore::new(1);
    b.iter(|| {
        sema.acquire();
        sema.release();
    });
}

#[bench]
fn sema_new_0(b: &mut Bencher) {
    b.iter(|| {
        let _sema = Semaphore::new(0);
    });
}

#[bench]
fn sema_new_1(b: &mut Bencher) {
    b.iter(|| {
        let _sema = Semaphore::new(1);
    });
}

#[bench]
fn sema_new_100(b: &mut Bencher) {
    b.iter(|| {
        let _sema = Semaphore::new(100);
    });
}
