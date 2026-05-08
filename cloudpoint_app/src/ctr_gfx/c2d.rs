//! Encapsulates the bindings we need to use citro2d (C lib provided by devkitpro) in Rust.
//! All of the internals are then abstracted via the ui module. Any additional calls needed
//! into citro2d as functionality expands should be covered by these bindings already.
//!
//! Generating the bindings (and compiling some "static inline" functions which need to be
//! recompiled once bindings are generated) should never need to be done again and is all
//! checked into VCS. But, if needed, see the `citro2d` recipe in `./justfile`.

#![allow(
    non_upper_case_globals,
    non_camel_case_types,
    non_snake_case,
    dead_code,
    unnecessary_transmutes,
    unsafe_op_in_unsafe_fn,
    clippy::all
)]
include!("./c2d/bindings.rs");
