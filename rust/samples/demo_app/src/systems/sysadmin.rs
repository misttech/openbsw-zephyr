// Copyright 2026 Mist Tecnologia LTDA. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

//! The system administration system, ported from `systems/SysAdminSystem.cpp`. It owns the
//! `lc` console command, which `main` registers; its 10 ms cycle does nothing.

use openbsw_async::{ContextType, QueueNode, Runnable, TimeUnit, Timeout};
use openbsw_lifecycle::{ComponentBase, LifecycleComponent};

const SYSTEM_CYCLE_TIME: u32 = 10;

/// Owns the lifecycle console command; its 10 ms cycle does nothing.
pub struct SysAdminSystem {
    base: ComponentBase,
    context: ContextType,
    timeout: Timeout,
    node: QueueNode<dyn Runnable>,
}

impl SysAdminSystem {
    /// A sysadmin system running on `context`.
    pub const fn new(context: ContextType) -> Self {
        Self {
            base: ComponentBase::with_context(context),
            context,
            timeout: Timeout::new(),
            node: QueueNode::new(),
        }
    }
}

impl LifecycleComponent for SysAdminSystem {
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

impl Runnable for SysAdminSystem {
    fn execute(&self) {}

    fn node(&self) -> &QueueNode<dyn Runnable> {
        &self.node
    }
}
