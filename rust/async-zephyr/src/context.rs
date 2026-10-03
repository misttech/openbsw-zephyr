// Copyright 2026 Mist Tecnologia LTDA. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

//! One async context on one Zephyr thread, ported from `async/TaskContext.h`.

use core::cell::Cell;
use core::ffi::{CStr, c_void};

use openbsw_async::{
    ContextType, Dispatcher, EventDispatcher, EventHandler, EventMaskType, EventPolicy, Runnable,
    RunnableExecutor, TimeUnit, Timeout,
};
use openbsw_timer::{Lock, Timer};

use crate::lock::LockType;

/// The number of events a context waits on: the runnable queue and the timer.
const EVENT_COUNT: usize = 2;
const RUNNABLE_EVENT: usize = 0;
const TIMER_EVENT: usize = 1;
const STOP_EVENT_MASK: EventMaskType = 1 << EVENT_COUNT;
const WAIT_EVENT_MASK: EventMaskType = (STOP_EVENT_MASK << 1) - 1;

/// What a task runs: the C++ `TaskFunctionType`, which by default dispatches events.
pub type TaskFunction = fn(&'static TaskContext);

/// The runnable queue, timer and event handling of one context.
///
/// A context is a `static` (its executor and timer link back to it); the
/// [`ZephyrAdapter`](crate::ZephyrAdapter) owns one per context.
pub struct TaskContext {
    dispatcher: EventDispatcher<EVENT_COUNT, LockType>,
    runnable_executor: RunnableExecutor<TaskContext, LockType, RUNNABLE_EVENT>,
    timer: Timer<LockType>,
    timer_event_policy: EventPolicy<TaskContext, TIMER_EVENT>,
    task_function: Cell<Option<TaskFunction>>,
    context: Cell<ContextType>,
    created: Cell<bool>,
}

// SAFETY: the cells are written while the adapter initializes the contexts, before the
// tasks start; afterwards they are only read.
unsafe impl Sync for TaskContext {}

/// The current time in microseconds, as `getSystemTimeUs32Bit`.
fn system_time_us() -> u32 {
    zephyr_ffi::cyc_to_us_floor32(zephyr_ffi::cycle_get_64())
}

impl TaskContext {
    /// A context, given its own `'static` address so that its executor and timer policy
    /// can signal it (the C++ members are constructed with `*this`).
    pub const fn new(this: &'static TaskContext) -> Self {
        Self {
            dispatcher: EventDispatcher::new(),
            runnable_executor: RunnableExecutor::new(this),
            timer: Timer::new(),
            timer_event_policy: EventPolicy::new(this),
            task_function: Cell::new(None),
            context: Cell::new(openbsw_async::CONTEXT_INVALID),
            created: Cell::new(false),
        }
    }

    /// Create the thread of `context` (not started): the port of `createTask` plus
    /// `createTimer`; the event and timer objects live in the shim.
    pub fn create_task(
        &'static self,
        context: ContextType,
        name: &'static CStr,
        priority: i32,
        task_function: Option<TaskFunction>,
    ) {
        self.context.set(context);
        self.task_function.set(Some(task_function.unwrap_or(Self::default_task_function)));
        self.runnable_executor.init();
        self.timer_event_policy.set_event_handler(self);
        zephyr_ffi::task_create(
            u32::from(context),
            name,
            priority,
            Self::static_task_function,
            self as *const Self as *mut c_void,
        );
        self.created.set(true);
    }

    /// Start the thread.
    pub fn start_task(&self) {
        if self.created.get() {
            zephyr_ffi::task_start(u32::from(self.context.get()));
        }
    }

    /// The thread's name.
    pub fn name(&self) -> &'static [u8] {
        if self.created.get() {
            zephyr_ffi::task_name(u32::from(self.context.get()))
        } else {
            b"<undefined>"
        }
    }

    /// The thread's stack size and used bytes, if Zephyr can tell.
    pub fn stack_usage(&self) -> Option<(u32, u32)> {
        let context = u32::from(self.context.get());
        let size = zephyr_ffi::task_stack_size(context);
        zephyr_ffi::task_stack_unused(context).map(|unused| (size, size - unused))
    }

    /// Queue `runnable` for execution.
    pub fn execute(&self, runnable: &'static dyn Runnable) {
        self.runnable_executor.enqueue(runnable);
    }

    /// Run `runnable` once after `delay` units, unless `timeout` is already set.
    pub fn schedule(
        &self,
        runnable: &'static dyn Runnable,
        timeout: &'static Timeout,
        delay: u32,
        unit: TimeUnit,
    ) {
        if !self.timer.is_active(timeout) {
            timeout.set_runnable(Some(runnable));
            timeout.set_context(self.context.get());
            if self.timer.set(timeout, delay.wrapping_mul(unit.microseconds()), system_time_us()) {
                self.timer_event_policy.set_event();
            }
        }
    }

    /// Run `runnable` every `period` units, unless `timeout` is already set.
    pub fn schedule_at_fixed_rate(
        &self,
        runnable: &'static dyn Runnable,
        timeout: &'static Timeout,
        period: u32,
        unit: TimeUnit,
    ) {
        if !self.timer.is_active(timeout) {
            timeout.set_runnable(Some(runnable));
            timeout.set_context(self.context.get());
            if self.timer.set_cyclic(
                timeout,
                period.wrapping_mul(unit.microseconds()),
                system_time_us(),
            ) {
                self.timer_event_policy.set_event();
            }
        }
    }

    /// Cancel `timeout`.
    pub fn cancel(&self, timeout: &Timeout) {
        self.timer.cancel(timeout);
    }

    /// Run the task function.
    pub fn call_task_function(&'static self) {
        if let Some(function) = self.task_function.get() {
            function(self);
        }
    }

    /// Wait for events and handle them until the stop event arrives.
    pub fn dispatch(&self) {
        let mut event_mask: EventMaskType = 0;
        while (event_mask & STOP_EVENT_MASK) == 0 {
            event_mask = self.wait_events();
            self.dispatcher.handle_events(event_mask);
        }
    }

    /// Stop dispatching: drop the runnable handler and wake the task with the stop event.
    pub fn stop_dispatch(&self) {
        self.runnable_executor.shutdown();
        self.set_events(STOP_EVENT_MASK);
    }

    /// Arm the timer for `time_in_us`, or signal the timer event at once for 0.
    pub fn set_timeout(&self, time_in_us: u32) {
        if time_in_us > 0 {
            zephyr_ffi::timer_start_us(u32::from(self.context.get()), time_in_us);
        } else {
            self.timer_event_policy.set_event();
        }
    }

    /// The timer expired: signal the timer event. Called from the shim's timer callback.
    pub fn timer_expired(&self) {
        self.timer_event_policy.set_event();
    }

    /// The default task function: dispatch forever.
    pub fn default_task_function(task_context: &'static TaskContext) {
        task_context.dispatch();
    }

    fn wait_events(&self) -> EventMaskType {
        let context = u32::from(self.context.get());
        let events = zephyr_ffi::event_wait(context, WAIT_EVENT_MASK);
        zephyr_ffi::event_clear(context, events);
        events
    }

    /// Run every expired timeout, then rearm the timer for the next one.
    fn handle_timeout(&self) {
        while self.timer.process_next_timeout(system_time_us()) {}
        if let Some(next_delta) = self.timer.get_next_delta(system_time_us()) {
            self.set_timeout(next_delta);
        }
    }

    extern "C" fn static_task_function(arg: *mut c_void) {
        // SAFETY: the shim passes back the pointer `create_task` gave it, which is the
        // address of a `'static` TaskContext.
        let task_context: &'static TaskContext = unsafe { &*(arg as *const TaskContext) };
        task_context.call_task_function();
    }
}

impl Dispatcher for TaskContext {
    fn set_event_handler(&self, event: usize, handler: &'static dyn EventHandler) {
        self.dispatcher.set_event_handler(event, handler);
    }

    fn remove_event_handler(&self, event: usize) {
        self.dispatcher.remove_event_handler(event);
    }

    fn set_events(&self, mask: EventMaskType) {
        zephyr_ffi::event_post(u32::from(self.context.get()), mask);
    }
}

/// The timer event's handler.
impl EventHandler for TaskContext {
    fn handle_event(&self) {
        self.handle_timeout();
    }
}

/// Lets the adapter take the lock type without naming it.
pub(crate) fn lock() -> LockType {
    LockType::lock()
}
