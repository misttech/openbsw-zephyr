// Copyright 2026 Mist Tecnologia LTDA. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

//! Port of `libs/asyncZephyr`: OpenBSW's async contexts on Zephyr.
//!
//! Each context is a Zephyr thread with an event object and a one-shot timer, all owned
//! by the application's C shim and reached through `zephyr_ffi`. A [`TaskContext`] runs
//! the context's loop: wait for its events, run the queued runnables (event bit 0), then
//! the expired timeouts (event bit 1) and rearm the timer. The [`ZephyrAdapter`] holds the
//! contexts and is the platform binding the `openbsw_async` functions reach.
//!
//! The order of everything here is the order of the C++ `TaskContext.h` and
//! `ZephyrAdapter.h`, since it decides the order of the demo's console lines.

#![no_std]
#![deny(unsafe_op_in_unsafe_fn)]
#![warn(missing_docs)]

mod adapter;
mod context;
mod hook;
mod lock;

pub use adapter::ZephyrAdapter;
pub use context::TaskContext;
pub use hook::{ContextHook, set_context_hook};
pub use lock::{IrqRawLock, LockType, ModifiableLockType};

#[cfg(test)]
mod tests;
