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

Sema provides a safe `Semaphore` abstraction built on the POSIX `sem_t` type.

[1]: https://cpjreynolds.github.io/sema
