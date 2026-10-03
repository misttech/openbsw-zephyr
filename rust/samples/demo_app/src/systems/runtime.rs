// Copyright 2026 Mist Tecnologia LTDA. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

//! The runtime statistics system, ported from `systems/RuntimeSystem.cpp`: owns the `stats`
//! command's snapshot, taken every second.

use openbsw_async::{ContextType, QueueNode, Runnable, TimeUnit, Timeout};
use openbsw_lifecycle::{ComponentBase, LifecycleComponent};

use crate::commands::statistics::StatisticsCommand;

const SYSTEM_CYCLE_TIME: u32 = 1000;

/// Snapshots the runtime statistics every second.
pub struct RuntimeSystem {
    base: ComponentBase,
    context: ContextType,
    timeout: Timeout,
    statistics_command: &'static StatisticsCommand,
    node: QueueNode<dyn Runnable>,
}

impl RuntimeSystem {
    /// A runtime system running on `context`, feeding `statistics_command`.
    pub const fn new(context: ContextType, statistics_command: &'static StatisticsCommand) -> Self {
        Self {
            base: ComponentBase::with_context(context),
            context,
            timeout: Timeout::new(),
            statistics_command,
            node: QueueNode::new(),
        }
    }
}

impl LifecycleComponent for RuntimeSystem {
    fn base(&self) -> &ComponentBase {
        &self.base
    }

    fn init(&'static self) {
        self.statistics_command.set_ticks_per_us(zephyr_ffi::cycles_per_sec() / 1_000_000);
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
        self.statistics_command.cyclic_1000ms();
    }

    fn node(&self) -> &QueueNode<dyn Runnable> {
        &self.node
    }
}
