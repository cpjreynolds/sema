# sema

Sema is an implementation of the Rust standard library `Semaphore` with POSIX
semaphores.

One major benefit of this implementation is that `Semaphore::release()` is
async-safe and may be called from within a signal handler, as defined in
POSIX.1-2001.

[![Build Status](https://travis-ci.org/cpjreynolds/sema.svg?branch=master)](https://travis-ci.org/cpjreynolds/sema) [![Crates.io](https://img.shields.io/crates/v/sema.svg)](https://crates.io/crates/sema) [![Crates.io](https://img.shields.io/crates/l/sema.svg)](https://crates.io/crates/sema)

[Documentation](https://cpjreynolds.github.io/sema)

## Usage

Add this to your `Cargo.toml`:

```toml
[dependencies]

sema = "*"
```

and this to your crate root:

```rust
extern crate sema;
```

