// Copyright 2026 Mist Tecnologia LTDA. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

//! The can system: the lifecycle transitions of `systems/CanSystem.cpp`. Its contents
//! follow with the library crate it is built on.

use openbsw_async::ContextType;
use openbsw_lifecycle::{ComponentBase, LifecycleComponent};

/// The can component.
pub struct CanSystem {
    base: ComponentBase,
}

impl CanSystem {
    /// A can system whose transitions run on `context`.
    pub const fn new(context: ContextType) -> Self {
        Self { base: ComponentBase::with_context(context) }
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
        self.transition_done();
    }

    fn shutdown(&'static self) {
        self.transition_done();
    }
}
