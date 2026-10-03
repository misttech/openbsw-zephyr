// Copyright 2026 Mist Tecnologia LTDA. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

//! The demo system, ported from `systems/DemoSystem.cpp` for a board with CAN and PWM and
//! without Ethernet or the potentiometer.
//!
//! Every 10 ms it moves the PWM LED one step along a sawtooth and, once a second has
//! passed, sends a counter in a CAN frame; it logs every frame received with an id from
//! 100 to 700.

use core::cell::Cell;

use openbsw_async::{ContextType, QueueNode, Runnable, TimeUnit, Timeout};
use openbsw_bsp_zephyr::system_timer::system_time_ms32;
use openbsw_cpp2can::filter::{Filter, IntervalFilter};
use openbsw_cpp2can::{CanFrame, CanFrameListener, ListenerNode};
use openbsw_lifecycle::{ComponentBase, LifecycleComponent};
use openbsw_util::{log_debug, log_info};

use super::can::CanSystem;
use crate::config::busid;
use crate::logger::DEMO;

const SYSTEM_CYCLE_TIME: u32 = 10;

/// Logs the frames with an id from 100 to 700: the port of `DemoSystem::CanReceiver`.
struct CanReceiver {
    filter: IntervalFilter,
    node: ListenerNode,
}

impl CanFrameListener for CanReceiver {
    fn frame_received(&self, frame: &CanFrame) {
        log_info!(DEMO, b"Frame received: 0x%x", frame.id());
    }

    fn filter(&self) -> &dyn Filter {
        &self.filter
    }

    fn node(&self) -> &ListenerNode {
        &self.node
    }
}

/// The demo application's component.
pub struct DemoSystem {
    base: ComponentBase,
    context: ContextType,
    timeout: Timeout,
    can_system: &'static CanSystem,
    can_receiver: CanReceiver,
    time_counter: Cell<u8>,
    /// The C++ `static previousSentTime`, initialized at the first cycle.
    previous_sent_time: Cell<Option<u32>>,
    can_sent_count: Cell<u32>,
    node: QueueNode<dyn Runnable>,
}

// SAFETY: the cells are only touched by `cyclic`, on the demo context.
unsafe impl Sync for DemoSystem {}

impl DemoSystem {
    /// A demo system running on `context`, sending through `can_system`.
    pub const fn new(context: ContextType, can_system: &'static CanSystem) -> Self {
        Self {
            base: ComponentBase::with_context(context),
            context,
            timeout: Timeout::new(),
            can_system,
            can_receiver: CanReceiver {
                filter: IntervalFilter::with_range(100, 700),
                node: ListenerNode::new(),
            },
            time_counter: Cell::new(0),
            previous_sent_time: Cell::new(None),
            can_sent_count: Cell::new(0),
            node: QueueNode::new(),
        }
    }

    /// The 10 ms cycle.
    pub fn cyclic(&self) {
        let time_counter = self.time_counter.get().wrapping_add(1);
        self.time_counter.set(time_counter);
        let _ = zephyr_ffi::pwm_set_led0(1_000_000, u32::from(time_counter) * 5000);
        let now = system_time_ms32();
        let previous_sent_time = match self.previous_sent_time.get() {
            Some(previous) => previous,
            None => {
                self.previous_sent_time.set(Some(now));
                now
            }
        };
        if now.wrapping_sub(previous_sent_time) >= 1000 {
            self.previous_sent_time.set(Some(now));
            if let Some(transceiver) = self.can_system.can_transceiver(busid::CAN_0) {
                let count = self.can_sent_count.get();
                log_debug!(DEMO, b"Sending frame %d", count);
                let mut frame = CanFrame::from_payload(0x558, &count.to_be_bytes());
                let _ = transceiver.write(&mut frame);
                self.can_sent_count.set(count.wrapping_add(1));
            }
        }
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
        if let Some(transceiver) = self.can_system.can_transceiver(busid::CAN_0) {
            transceiver.add_can_frame_listener(&self.can_receiver);
        }
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
