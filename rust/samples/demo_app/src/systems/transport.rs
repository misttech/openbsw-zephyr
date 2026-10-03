// Copyright 2026 Mist Tecnologia LTDA. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

//! The transport system, ported from `systems/TransportSystem.cpp` and
//! `transport/ITransportSystem.h`: owns the router the transport layers plug into. The
//! router as a message provider follows with the UDS system.

use openbsw_async::ContextType;
use openbsw_async_zephyr::LockType;
use openbsw_lifecycle::{ComponentBase, LifecycleComponent};
use openbsw_transport::{RouterConfig, TransportLayer, TransportRouterSimple};

use crate::config::{busid, transport_configuration};

/// The transport component.
pub struct TransportSystem {
    base: ComponentBase,
    transport_router: TransportRouterSimple<LockType>,
}

impl TransportSystem {
    /// A transport system whose transitions run on `context`, given its own `'static`
    /// address (the router's messages are over its buffers).
    pub const fn new(this: &'static Self, context: ContextType) -> Self {
        Self {
            base: ComponentBase::with_context(context),
            transport_router: TransportRouterSimple::new(
                &this.transport_router,
                RouterConfig {
                    selfdiag_bus_id: busid::SELFDIAG,
                    can_bus_id: busid::CAN_0,
                    functional_address: transport_configuration::FUNCTIONAL_ALL_ISO14229 as u8,
                    max_functional_payload:
                        transport_configuration::MAX_FUNCTIONAL_MESSAGE_PAYLOAD_SIZE,
                },
            ),
        }
    }

    /// Route messages to and from `layer` (`ITransportSystem::addTransportLayer`).
    pub fn add_transport_layer(&'static self, layer: &'static dyn TransportLayer) {
        self.transport_router.add_transport_layer(layer);
    }

    /// Stop routing to `layer` (`ITransportSystem::removeTransportLayer`).
    pub fn remove_transport_layer(&self, layer: &'static dyn TransportLayer) {
        self.transport_router.remove_transport_layer(layer);
    }
}

impl LifecycleComponent for TransportSystem {
    fn base(&self) -> &ComponentBase {
        &self.base
    }

    fn init(&'static self) {
        self.transport_router.init();
        self.transition_done();
    }

    fn run(&'static self) {
        self.transition_done();
    }

    fn shutdown(&'static self) {
        self.transition_done();
    }
}
