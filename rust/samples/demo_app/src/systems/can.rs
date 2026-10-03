// Copyright 2026 Mist Tecnologia LTDA. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

//! The CAN system, ported from `systems/CanSystem.cpp` and `systems/ICanSystem.h`: owns
//! the transceiver of the one bus and opens it at its run level.

use openbsw_async::ContextType;
use openbsw_bsp_zephyr::ZephyrCanTransceiver;
use openbsw_cpp2can::CanTransceiver;
use openbsw_lifecycle::{ComponentBase, LifecycleComponent};

use crate::config::busid;

/// The CAN component.
pub struct CanSystem {
    base: ComponentBase,
    transceiver0: &'static ZephyrCanTransceiver,
}

impl CanSystem {
    /// A CAN system whose transitions run on `context`, over `transceiver0` (the C++
    /// member is a separate `static` here, since its runnables keep its address).
    pub const fn new(context: ContextType, transceiver0: &'static ZephyrCanTransceiver) -> Self {
        Self { base: ComponentBase::with_context(context), transceiver0 }
    }

    /// The transceiver of `bus_id`, if this system has one (`ICanSystem::getCanTransceiver`).
    pub fn can_transceiver(&self, bus_id: u8) -> Option<&'static dyn CanTransceiver> {
        (bus_id == busid::CAN_0).then_some(self.transceiver0 as &'static dyn CanTransceiver)
    }
}

impl LifecycleComponent for CanSystem {
    fn base(&self) -> &ComponentBase {
        &self.base
    }

    fn init(&'static self) {
        self.transition_done();
    }

    fn run(&'static self) {
        let _ = self.transceiver0.init();
        let _ = self.transceiver0.open();
        self.transition_done();
    }

    fn shutdown(&'static self) {
        let _ = self.transceiver0.close();
        self.transceiver0.shutdown();
        self.transition_done();
    }
}
