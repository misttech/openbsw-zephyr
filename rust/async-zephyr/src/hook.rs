// Copyright 2026 Mist Tecnologia LTDA. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

//! The hooks Zephyr's tracing calls on every context switch and interrupt, ported from
//! `async/Hook.h` and `async/StaticContextHook.h`.
//!
//! The shim defines `sys_trace_*_user` and forwards to the `rust_async_*` functions here,
//! which forward to the installed [`ContextHook`] (the runtime monitor).

use openbsw_util::cell::RacyCell;

/// What the hooks report to: the port of `StaticContextHook<RuntimeMonitor>`.
pub trait ContextHook: Sync {
    /// The task `task_idx` was switched in.
    fn enter_task(&self, task_idx: usize);
    /// The task `task_idx` was switched out.
    fn leave_task(&self, task_idx: usize);
    /// An interrupt of `isr_group_idx` began.
    fn enter_isr_group(&self, isr_group_idx: usize);
    /// An interrupt of `isr_group_idx` ended.
    fn leave_isr_group(&self, isr_group_idx: usize);
}

static HOOK: RacyCell<Option<&'static dyn ContextHook>> = RacyCell::new(None);

/// Install the context hook. Call during startup, before the tasks run.
pub fn set_context_hook(hook: &'static dyn ContextHook) {
    // SAFETY: called during single-threaded startup, before any hook fires.
    unsafe { HOOK.set(Some(hook)) };
}

fn hook() -> Option<&'static dyn ContextHook> {
    // SAFETY: `set_context_hook` runs before the tasks start; afterwards only reads.
    *unsafe { HOOK.get() }
}

/// A task was switched in; called by the shim with the task's context index.
#[unsafe(no_mangle)]
pub extern "C" fn rust_async_enter_task(task_idx: i32) {
    if let (Some(hook), Ok(idx)) = (hook(), usize::try_from(task_idx)) {
        hook.enter_task(idx);
    }
}

/// A task was switched out.
#[unsafe(no_mangle)]
pub extern "C" fn rust_async_leave_task(task_idx: i32) {
    if let (Some(hook), Ok(idx)) = (hook(), usize::try_from(task_idx)) {
        hook.leave_task(idx);
    }
}

/// An interrupt of a group began.
#[unsafe(no_mangle)]
pub extern "C" fn rust_async_enter_isr_group(isr_group_idx: u32) {
    if let Some(hook) = hook() {
        hook.enter_isr_group(isr_group_idx as usize);
    }
}

/// An interrupt of a group ended.
#[unsafe(no_mangle)]
pub extern "C" fn rust_async_leave_isr_group(isr_group_idx: u32) {
    if let Some(hook) = hook() {
        hook.leave_isr_group(isr_group_idx as usize);
    }
}
