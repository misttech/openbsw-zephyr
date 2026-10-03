// Copyright 2026 Mist Tecnologia LTDA. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

//! The adapter on the host: Zephyr is a recording `Hooks` implementation, and the tests
//! drive the task loop by handing it the events a thread would wake on.

extern crate std;

use core::ffi::{CStr, c_void};
use core::sync::atomic::{AtomicBool, AtomicI32, AtomicU64, AtomicUsize, Ordering};
use std::collections::VecDeque;
use std::sync::Mutex;
use std::vec::Vec;

use openbsw_async::{Async, CONTEXT_INVALID, QueueNode, Runnable, TimeUnit, Timeout};
use zephyr_ffi::TaskEntry;
use zephyr_ffi::host::Hooks;

use crate::adapter::rust_task_timer_expired;
use crate::hook::{
    ContextHook, rust_async_enter_isr_group, rust_async_enter_task, rust_async_leave_isr_group,
    rust_async_leave_task, set_context_hook,
};
use crate::{TaskContext, ZephyrAdapter};

/// The hooks are process-wide, so the tests run one at a time.
static SERIAL: Mutex<()> = Mutex::new(());

#[derive(Debug, PartialEq, Eq, Clone)]
enum Call {
    TaskCreate(u32, &'static CStr, i32),
    TaskStart(u32),
    EventPost(u32, u32),
    EventWait(u32, u32),
    EventClear(u32, u32),
    TimerStart(u32, u32),
    TimerStop(u32),
}

/// Zephyr as the tests see it: records every call, answers `event_wait` from a queue and
/// the clock from a counter at 1 MHz.
struct Zephyr {
    calls: Mutex<Vec<Call>>,
    wake_events: Mutex<VecDeque<u32>>,
    names: Mutex<Vec<(u32, &'static CStr)>>,
    now_us: AtomicU64,
    priority: AtomicI32,
    in_isr: AtomicBool,
}

static ZEPHYR: Zephyr = Zephyr {
    calls: Mutex::new(Vec::new()),
    wake_events: Mutex::new(VecDeque::new()),
    names: Mutex::new(Vec::new()),
    now_us: AtomicU64::new(0),
    priority: AtomicI32::new(0),
    in_isr: AtomicBool::new(false),
};

impl Zephyr {
    fn record(&self, call: Call) {
        self.calls.lock().unwrap().push(call);
    }

    fn take(&self) -> Vec<Call> {
        core::mem::take(&mut *self.calls.lock().unwrap())
    }

    /// Start a test: install the hooks and forget the previous test.
    fn start() -> std::sync::MutexGuard<'static, ()> {
        let guard = SERIAL.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        zephyr_ffi::host::install(&ZEPHYR);
        ZEPHYR.take();
        ZEPHYR.wake_events.lock().unwrap().clear();
        ZEPHYR.now_us.store(0, Ordering::Relaxed);
        ZEPHYR.priority.store(0, Ordering::Relaxed);
        ZEPHYR.in_isr.store(false, Ordering::Relaxed);
        guard
    }

    /// The events the next `event_wait` calls return, in order.
    fn wake_with(&self, events: &[u32]) {
        self.wake_events.lock().unwrap().extend(events.iter().copied());
    }
}

impl Hooks for Zephyr {
    fn task_create(
        &self,
        context: u32,
        name: &'static CStr,
        priority: i32,
        _entry: TaskEntry,
        _arg: *mut c_void,
    ) {
        self.names.lock().unwrap().push((context, name));
        self.record(Call::TaskCreate(context, name, priority));
    }
    fn task_start(&self, context: u32) {
        self.record(Call::TaskStart(context));
    }
    fn task_name(&self, context: u32) -> &'static [u8] {
        self.names
            .lock()
            .unwrap()
            .iter()
            .rev()
            .find(|(c, _)| *c == context)
            .map_or(b"<undefined>", |(_, name)| name.to_bytes())
    }
    fn event_post(&self, context: u32, mask: u32) {
        self.record(Call::EventPost(context, mask));
    }
    fn event_wait(&self, context: u32, mask: u32) -> u32 {
        self.record(Call::EventWait(context, mask));
        self.wake_events
            .lock()
            .unwrap()
            .pop_front()
            .expect("the task waits with nothing to wake it")
    }
    fn event_clear(&self, context: u32, mask: u32) {
        self.record(Call::EventClear(context, mask));
    }
    fn timer_start_us(&self, context: u32, microseconds: u32) {
        self.record(Call::TimerStart(context, microseconds));
    }
    fn timer_stop(&self, context: u32) {
        self.record(Call::TimerStop(context));
    }
    fn is_in_isr(&self) -> bool {
        self.in_isr.load(Ordering::Relaxed)
    }
    fn current_priority(&self) -> i32 {
        self.priority.load(Ordering::Relaxed)
    }
    fn cycle_get_64(&self) -> u64 {
        self.now_us.load(Ordering::Relaxed)
    }
}

/// Counts its executions.
struct Counter {
    count: AtomicUsize,
    node: QueueNode<dyn Runnable>,
}

impl Counter {
    const fn new() -> Self {
        Self { count: AtomicUsize::new(0), node: QueueNode::new() }
    }

    fn take(&self) -> usize {
        self.count.swap(0, Ordering::Relaxed)
    }
}

impl Runnable for Counter {
    fn execute(&self) {
        self.count.fetch_add(1, Ordering::Relaxed);
    }
    fn node(&self) -> &QueueNode<dyn Runnable> {
        &self.node
    }
}

const RUNNABLE_EVENT: u32 = 1 << 0;
const TIMER_EVENT: u32 = 1 << 1;
const STOP_EVENT: u32 = 1 << 2;
const WAIT_MASK: u32 = 0b111;

static CONTEXTS: [TaskContext; 2] =
    [TaskContext::new(&CONTEXTS[0]), TaskContext::new(&CONTEXTS[1])];
static ADAPTER: ZephyrAdapter<2> = ZephyrAdapter::new(&CONTEXTS, [c"first", c"second"]);
static RUNNABLE: Counter = Counter::new();
static TIMEOUT: Timeout = Timeout::new();

#[test]
fn init_creates_a_thread_per_context_and_run_starts_them() {
    let _serial = Zephyr::start();
    ADAPTER.init();
    assert_eq!(
        ZEPHYR.take(),
        [Call::TaskCreate(0, c"first", 1), Call::TaskCreate(1, c"second", 2)]
    );
    ADAPTER.run();
    assert_eq!(ZEPHYR.take(), [Call::TaskStart(0), Call::TaskStart(1)]);
    assert_eq!(ADAPTER.task_name(0), b"first");
    assert_eq!(ADAPTER.task_name(1), b"second");
    assert_eq!(ADAPTER.task_name(2), b"<undefined>");
}

#[test]
fn execute_posts_the_runnable_event_and_dispatch_runs_it() {
    let _serial = Zephyr::start();
    ADAPTER.init();
    ZEPHYR.take();
    ADAPTER.execute(1, &RUNNABLE);
    assert_eq!(ZEPHYR.take(), [Call::EventPost(1, RUNNABLE_EVENT)]);
    assert_eq!(RUNNABLE.take(), 0);
    // The thread wakes on the runnable event, then on the stop event.
    ZEPHYR.wake_with(&[RUNNABLE_EVENT, STOP_EVENT]);
    ADAPTER.context(1).dispatch();
    assert_eq!(RUNNABLE.take(), 1);
    assert_eq!(
        ZEPHYR.take(),
        [
            Call::EventWait(1, WAIT_MASK),
            Call::EventClear(1, RUNNABLE_EVENT),
            Call::EventWait(1, WAIT_MASK),
            Call::EventClear(1, STOP_EVENT),
        ]
    );
}

#[test]
fn schedule_arms_the_timer_and_runs_the_runnable_once_it_expired() {
    let _serial = Zephyr::start();
    ADAPTER.init();
    ZEPHYR.take();
    ADAPTER.schedule(0, &RUNNABLE, &TIMEOUT, 5, TimeUnit::Milliseconds);
    // A second schedule of an active timeout is ignored, as in C++.
    ADAPTER.schedule(0, &RUNNABLE, &TIMEOUT, 1, TimeUnit::Milliseconds);
    assert_eq!(ZEPHYR.take(), [Call::EventPost(0, TIMER_EVENT)]);
    // Handling the timer event before the timeout is due arms the one-shot timer.
    ZEPHYR.wake_with(&[TIMER_EVENT, STOP_EVENT]);
    ADAPTER.context(0).dispatch();
    assert_eq!(RUNNABLE.take(), 0);
    assert!(ZEPHYR.take().contains(&Call::TimerStart(0, 5000)));
    // The shim's timer callback signals the event; the runnable runs and nothing is rearmed.
    ZEPHYR.now_us.store(5000, Ordering::Relaxed);
    rust_task_timer_expired(0);
    assert_eq!(ZEPHYR.take(), [Call::EventPost(0, TIMER_EVENT)]);
    ZEPHYR.wake_with(&[TIMER_EVENT, STOP_EVENT]);
    ADAPTER.context(0).dispatch();
    assert_eq!(RUNNABLE.take(), 1);
    assert!(!ZEPHYR.take().iter().any(|call| matches!(call, Call::TimerStart(..))));
    // The context of an unknown timer is ignored.
    rust_task_timer_expired(7);
    assert_eq!(ZEPHYR.take(), []);
}

#[test]
fn schedule_at_fixed_rate_rearms_from_the_previous_expiry_until_cancelled() {
    let _serial = Zephyr::start();
    ADAPTER.init();
    ZEPHYR.take();
    ADAPTER.schedule_at_fixed_rate(0, &RUNNABLE, &TIMEOUT, 10, TimeUnit::Milliseconds);
    ZEPHYR.wake_with(&[TIMER_EVENT, STOP_EVENT]);
    ADAPTER.context(0).dispatch();
    assert!(ZEPHYR.take().contains(&Call::TimerStart(0, 10_000)));
    // Late by 300 us: the next period still ends at 20 ms.
    ZEPHYR.now_us.store(10_300, Ordering::Relaxed);
    ZEPHYR.wake_with(&[TIMER_EVENT, STOP_EVENT]);
    ADAPTER.context(0).dispatch();
    assert_eq!(RUNNABLE.take(), 1);
    assert!(ZEPHYR.take().contains(&Call::TimerStart(0, 9_700)));
    assert_eq!(TIMEOUT.context(), 0);
    ADAPTER.cancel(&TIMEOUT);
    assert_eq!(TIMEOUT.context(), CONTEXT_INVALID);
    ZEPHYR.now_us.store(20_000, Ordering::Relaxed);
    ZEPHYR.wake_with(&[TIMER_EVENT, STOP_EVENT]);
    ADAPTER.context(0).dispatch();
    assert_eq!(RUNNABLE.take(), 0);
    // Cancelling again is harmless.
    ADAPTER.cancel(&TIMEOUT);
}

#[test]
fn current_context_is_the_priority_minus_one_and_invalid_in_an_interrupt() {
    let _serial = Zephyr::start();
    ZEPHYR.priority.store(3, Ordering::Relaxed);
    assert_eq!(ADAPTER.current_context(), 2);
    ZEPHYR.in_isr.store(true, Ordering::Relaxed);
    assert_eq!(ADAPTER.current_context(), CONTEXT_INVALID);
}

#[test]
fn stop_dispatch_posts_the_stop_event() {
    let _serial = Zephyr::start();
    ADAPTER.init();
    ZEPHYR.take();
    ADAPTER.context(1).stop_dispatch();
    assert_eq!(ZEPHYR.take(), [Call::EventPost(1, STOP_EVENT)]);
}

struct HookRecorder(Mutex<Vec<(&'static str, usize)>>);

impl ContextHook for HookRecorder {
    fn enter_task(&self, task_idx: usize) {
        self.0.lock().unwrap().push(("enter_task", task_idx));
    }
    fn leave_task(&self, task_idx: usize) {
        self.0.lock().unwrap().push(("leave_task", task_idx));
    }
    fn enter_isr_group(&self, isr_group_idx: usize) {
        self.0.lock().unwrap().push(("enter_isr", isr_group_idx));
    }
    fn leave_isr_group(&self, isr_group_idx: usize) {
        self.0.lock().unwrap().push(("leave_isr", isr_group_idx));
    }
}

static HOOK: HookRecorder = HookRecorder(Mutex::new(Vec::new()));

#[test]
fn tracing_hooks_reach_the_context_hook_and_skip_foreign_threads() {
    let _serial = Zephyr::start();
    // Before a hook is set, the calls are dropped.
    rust_async_enter_task(0);
    set_context_hook(&HOOK);
    rust_async_enter_task(1);
    rust_async_enter_isr_group(0);
    rust_async_leave_isr_group(0);
    rust_async_leave_task(1);
    // The shim passes -1 for threads that are not async contexts.
    rust_async_enter_task(-1);
    rust_async_leave_task(-1);
    assert_eq!(
        core::mem::take(&mut *HOOK.0.lock().unwrap()),
        [("enter_task", 1), ("enter_isr", 0), ("leave_isr", 0), ("leave_task", 1)]
    );
}
