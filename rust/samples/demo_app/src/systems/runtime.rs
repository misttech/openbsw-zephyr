// Copyright 2026 Mist Tecnologia LTDA. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

//! The runtime statistics system, ported from `systems/RuntimeSystem.cpp`.
//!
//! The statistics command and the 1000 ms snapshot follow with the `runtime` crate; the
//! transitions and the cyclic runnable are in place.

use openbsw_async::{ContextType, QueueNode, Runnable, TimeUnit, Timeout};
use openbsw_lifecycle::{ComponentBase, LifecycleComponent};

const SYSTEM_CYCLE_TIME: u32 = 1000;

/// Snapshots the runtime statistics every second.
pub struct RuntimeSystem {
    base: ComponentBase,
    context: ContextType,
    timeout: Timeout,
    node: QueueNode<dyn Runnable>,
}

impl RuntimeSystem {
    /// A runtime system running on `context`.
    pub const fn new(context: ContextType) -> Self {
        Self {
            base: ComponentBase::with_context(context),
            context,
            timeout: Timeout::new(),
            node: QueueNode::new(),
        }
    }
}

impl LifecycleComponent for RuntimeSystem {
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
        self.timeout.cancel();
        self.transition_done();
    }
}

impl Runnable for RuntimeSystem {
    fn execute(&self) {
        // StatisticsCommand::cyclic_1000ms follows with the runtime crate.
    }

    fn node(&self) -> &QueueNode<dyn Runnable> {
        &self.node
    }
}
