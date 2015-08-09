sema
====

A simple semaphore.

[![Build Status](https://travis-ci.org/cpjreynolds/sema.svg?branch=master)](https://travis-ci.org/cpjreynolds/sema) [![Crates.io](https://img.shields.io/crates/v/sema.svg)](https://crates.io/crates/sema) [![Crates.io](https://img.shields.io/crates/l/sema.svg)](https://crates.io/crates/sema)

[Documentation][1]

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

## Overview

Sema provides a safe `Semaphore` implementation.

## Implementation

Sema has the same semantics on all supported platforms, however due to platform
differences, the implementation differs between them.

### Linux

On Linux, `Semaphore`s are implemented with futexes. They are based on the
current glibc `sem_t` implementation and share the same semantics.

### OS X

OS X does not implement unnamed semaphores, however it does implement named
semaphores, which share the same semantics as their unnamed counterparts but may
be shared between processes.

Sema implements pseudo-unnamed semaphores with randomly named semaphores. Since
the semantics of their operations remain the same, the only difference is their
construction and destruction, however this is transparent to a consumer of this
library.

### Other Platforms

Sema should, in theory, work on any platform that supports POSIX semaphores (or
futexes, in the case of Linux). That being said, it would be wise to consult
your platform's semaphore manpages just in case.

[1]: https://cpjreynolds.github.io/sema
