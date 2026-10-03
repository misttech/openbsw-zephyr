// Copyright 2026 Mist Tecnologia LTDA. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

//! The demo system, ported from `systems/DemoSystem.cpp`.
//!
//! Every 10 ms it moves the PWM LED one step along a sawtooth; the CAN frame once a
//! second and the frame listener follow with the `cpp2can` crate.

use core::cell::Cell;

use openbsw_async::{ContextType, QueueNode, Runnable, TimeUnit, Timeout};
use openbsw_lifecycle::{ComponentBase, LifecycleComponent};

const SYSTEM_CYCLE_TIME: u32 = 10;

/// The demo application's component.
pub struct DemoSystem {
    base: ComponentBase,
    context: ContextType,
    timeout: Timeout,
    time_counter: Cell<u8>,
    node: QueueNode<dyn Runnable>,
}

// SAFETY: the counter is only touched by `cyclic`, on the demo context.
unsafe impl Sync for DemoSystem {}

impl DemoSystem {
    /// A demo system running on `context`.
    pub const fn new(context: ContextType) -> Self {
        Self {
            base: ComponentBase::with_context(context),
            context,
            timeout: Timeout::new(),
            time_counter: Cell::new(0),
            node: QueueNode::new(),
        }
    }

    /// The 10 ms cycle.
    pub fn cyclic(&self) {
        let time_counter = self.time_counter.get().wrapping_add(1);
        self.time_counter.set(time_counter);
        zephyr_ffi::pwm_set_led0(1_000_000, u32::from(time_counter) * 5000);
    }
}

impl LifecycleComponent for DemoSystem {
    fn base(&self) -> &ComponentBase {
        &self.base
    }

    fn init(&'static self) {
        self.transition_done();
    }

    fn run(&'static self) {
        openbsw_async::schedule_at_fixed_rate(
            self.context,
            self,
            &self.timeout,
            SYSTEM_CYCLE_TIME,
            TimeUnit::Milliseconds,
        );
        self.transition_done();
    }

    fn shutdown(&'static self) {
        self.transition_done();
    }
}

impl Runnable for DemoSystem {
    fn execute(&self) {
        self.cyclic();
    }

    fn node(&self) -> &QueueNode<dyn Runnable> {
        &self.node
    }
}
