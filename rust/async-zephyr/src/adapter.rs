// Copyright 2026 Mist Tecnologia LTDA. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

//! The platform binding, ported from `async/ZephyrAdapter.h`.

use core::ffi::CStr;

use openbsw_async::{Async, CONTEXT_INVALID, ContextType, Runnable, TimeUnit, Timeout};
use openbsw_util::cell::RacyCell;

use crate::context::TaskContext;

/// The contexts the shim's timer callback reaches, set by `ZephyrAdapter::init`.
static CONTEXTS: RacyCell<Option<&'static [TaskContext]>> = RacyCell::new(None);

/// The one-shot timer of `context` expired; called by the shim's timer callback.
#[unsafe(no_mangle)]
pub extern "C" fn rust_task_timer_expired(context: u32) {
    // SAFETY: `init` sets the contexts during single-threaded startup; afterwards only reads.
    if let Some(contexts) = *unsafe { CONTEXTS.get() }
        && let Some(task_context) = contexts.get(context as usize)
    {
        task_context.timer_expired();
    }
}

/// The `N` contexts of the application as Zephyr threads: the port of
/// `ZephyrAdapter<Binding>`. Context `i` runs at Zephyr priority `i + 1`.
pub struct ZephyrAdapter<const N: usize> {
    contexts: &'static [TaskContext; N],
    names: [&'static CStr; N],
}

impl<const N: usize> ZephyrAdapter<N> {
    /// An adapter over `contexts`, naming the threads `names`.
    pub const fn new(contexts: &'static [TaskContext; N], names: [&'static CStr; N]) -> Self {
        Self { contexts, names }
    }

    /// The context with index `idx`.
    pub fn context(&self, idx: ContextType) -> &'static TaskContext {
        &self.contexts[usize::from(idx)]
    }

    /// Create every thread, suspended: the port of `init`, which ran the task initializers.
    pub fn init(&self) {
        // SAFETY: called during single-threaded startup, before the timers can fire.
        unsafe { CONTEXTS.set(Some(self.contexts)) };
        for (idx, context) in self.contexts.iter().enumerate() {
            // Context ids start with 0; priority 1 is the highest.
            context.create_task(idx as ContextType, self.names[idx], idx as i32 + 1, None);
        }
    }

    /// Start every thread.
    pub fn run(&self) {
        for context in self.contexts.iter() {
            context.start_task();
        }
    }

    /// The stack size and used bytes of the thread of `idx`.
    pub fn stack_usage(&self, idx: ContextType) -> Option<(u32, u32)> {
        self.contexts.get(usize::from(idx)).and_then(|context| context.stack_usage())
    }

    /// The context whose thread is running, `CONTEXT_INVALID` in an interrupt.
    pub fn current_task_context() -> ContextType {
        if zephyr_ffi::is_in_isr() {
            return CONTEXT_INVALID;
        }
        (zephyr_ffi::current_priority() - 1) as ContextType
    }
}

impl<const N: usize> Async for ZephyrAdapter<N> {
    fn execute(&self, context: ContextType, runnable: &'static dyn Runnable) {
        self.contexts[usize::from(context)].execute(runnable);
    }

    fn schedule(
        &self,
        context: ContextType,
        runnable: &'static dyn Runnable,
        timeout: &'static Timeout,
        delay: u32,
        unit: TimeUnit,
    ) {
        self.contexts[usize::from(context)].schedule(runnable, timeout, delay, unit);
    }

    fn schedule_at_fixed_rate(
        &self,
        context: ContextType,
        runnable: &'static dyn Runnable,
        timeout: &'static Timeout,
        period: u32,
        unit: TimeUnit,
    ) {
        self.contexts[usize::from(context)].schedule_at_fixed_rate(runnable, timeout, period, unit);
    }

    fn cancel(&self, timeout: &'static Timeout) {
        let _lock = crate::context::lock();
        let context = timeout.context();
        if context != CONTEXT_INVALID {
            timeout.set_context(CONTEXT_INVALID);
            self.contexts[usize::from(context)].cancel(timeout);
        }
    }

    fn current_context(&self) -> ContextType {
        Self::current_task_context()
    }

    fn task_name(&self, context: ContextType) -> &'static [u8] {
        self.contexts.get(usize::from(context)).map_or(b"<undefined>", |c| c.name())
    }
}
