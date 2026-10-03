// Copyright 2026 Mist Tecnologia LTDA. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

//! The DoCAN system, ported from `systems/DoCanSystem.cpp`: one ISO 15765-2 transport
//! layer on the CAN bus with normal addressing and the padded classic codec, registered
//! with the transport system at its run level, driven by a 10 ms cyclic task and, while
//! consecutive frames are being paced, by 200 us ticks.

use openbsw_async::{ContextType, QueueNode, Runnable, TimeUnit, Timeout};
use openbsw_async_zephyr::LockType;
use openbsw_bsp_zephyr::system_timer::system_time_us32;
use openbsw_cpp2can::CanTransceiver;
use openbsw_docan::codec::{FdFrameSizeMapper, presets};
use openbsw_docan::{
    AddressEntry, DOCAN, DoCanParameters, DoCanTransportLayer, DoCanTransportLayerConfig,
    DoCanTransportLayerContainer, FrameCodec, FrameCodecConfig, NormalAddressingFilter,
    PhysicalCanTransceiver, TickGenerator,
};
use openbsw_lifecycle::{ComponentBase, LifecycleComponent};

use super::transport::TransportSystem;
use crate::config::{LOGICAL_ADDRESS, busid};

/// The cyclic task's period, in milliseconds.
const TIMEOUT_DOCAN_SYSTEM: u32 = 10;
/// Ticks are `TICK_DELTA_TICKS` times 100 us apart.
const TICK_DELTA_TICKS: u32 = 2;
const ALLOCATE_TIMEOUT: u16 = 1000;
const RX_TIMEOUT: u16 = 1000;
const TX_CALLBACK_TIMEOUT: u16 = 1000;
const FLOW_CONTROL_TIMEOUT: u16 = 1000;
const ALLOCATE_RETRY_COUNT: u8 = 15;
const FLOW_CONTROL_WAIT_COUNT: u8 = 15;
const MIN_SEPARATION_TIME: u32 = 2;
const BLOCK_SIZE: u8 = 15;

/// The number of receivers and transmitters the layer can hold at once.
const RX_COUNT: usize = 80;
const TX_COUNT: usize = 15;

/// The address table: requests on 0x02A from tester 0x0F0 to this ECU, answered on 0x0F0.
static ADDRESSES: [AddressEntry; 1] = [AddressEntry {
    can_reception_id: 0x02A,
    can_transmission_id: 0x0F0,
    transport_source_id: 0x0F0,
    transport_target_id: LOGICAL_ADDRESS,
    reception_codec_idx: 0,
    transmission_codec_idx: 0,
}];
static FRAME_SIZE_MAPPER: FdFrameSizeMapper = FdFrameSizeMapper;
static CLASSIC_CODEC_CONFIG: FrameCodecConfig = presets::PADDED_CLASSIC;
static CLASSIC_CODEC: FrameCodec = FrameCodec::new(&CLASSIC_CODEC_CONFIG, &FRAME_SIZE_MAPPER);
static CODECS: [Option<&FrameCodec>; 1] = [Some(&CLASSIC_CODEC)];
static CLASSIC_ADDRESSING_FILTER: NormalAddressingFilter = NormalAddressingFilter::new();
static PARAMETERS: DoCanParameters = DoCanParameters::new(
    &system_time_us32,
    ALLOCATE_TIMEOUT,
    RX_TIMEOUT,
    TX_CALLBACK_TIMEOUT,
    FLOW_CONTROL_TIMEOUT,
    ALLOCATE_RETRY_COUNT,
    FLOW_CONTROL_WAIT_COUNT,
    MIN_SEPARATION_TIME,
    BLOCK_SIZE,
);
/// The pools: a `static` of its own, since the layer reads its parameters while the
/// system is built.
static TRANSPORT_LAYER_CONFIG: DoCanTransportLayerConfig<RX_COUNT, TX_COUNT> =
    DoCanTransportLayerConfig::new(&PARAMETERS);

/// Asks for ticks while consecutive frames are being paced: the port of
/// `DoCanSystem::TickGeneratorRunnableAdapter`.
struct TickGeneratorRunnableAdapter {
    this: &'static Self,
    layers: &'static DoCanTransportLayerContainer<LockType>,
    tick_timeout: Timeout,
    context: ContextType,
    node: QueueNode<dyn Runnable>,
}

impl TickGeneratorRunnableAdapter {
    fn schedule_tick(&self) {
        openbsw_async::schedule(
            self.context,
            self.this,
            &self.this.tick_timeout,
            TICK_DELTA_TICKS * 100,
            TimeUnit::Microseconds,
        );
    }
}

impl TickGenerator for TickGeneratorRunnableAdapter {
    fn tick_needed(&self) {
        self.schedule_tick();
    }
}

impl Runnable for TickGeneratorRunnableAdapter {
    fn execute(&self) {
        if self.layers.tick(system_time_us32()) {
            self.schedule_tick();
        }
    }

    fn node(&self) -> &QueueNode<dyn Runnable> {
        &self.node
    }
}

/// The DoCAN component.
pub struct DoCanSystem {
    base: ComponentBase,
    context: ContextType,
    cyclic_timeout: Timeout,
    transport_system: &'static TransportSystem,
    physical_transceiver: PhysicalCanTransceiver,
    transport_layer: DoCanTransportLayer<LockType>,
    layers: [&'static DoCanTransportLayer<LockType>; 1],
    transport_layers: DoCanTransportLayerContainer<LockType>,
    tick_generator: TickGeneratorRunnableAdapter,
    node: QueueNode<dyn Runnable>,
}

impl DoCanSystem {
    /// A DoCAN system whose transitions and tasks run on `context`, given its own
    /// `'static` address, over the CAN system's `transceiver` of `busid::CAN_0` (the C++
    /// constructor looks it up at init; here the layer is built at compile time).
    pub const fn new(
        this: &'static Self,
        transport_system: &'static TransportSystem,
        transceiver: &'static dyn CanTransceiver,
        context: ContextType,
    ) -> Self {
        Self {
            base: ComponentBase::with_context(context),
            context,
            cyclic_timeout: Timeout::new(),
            transport_system,
            physical_transceiver: PhysicalCanTransceiver::new(
                &this.physical_transceiver,
                transceiver,
                &CLASSIC_ADDRESSING_FILTER,
                &CLASSIC_ADDRESSING_FILTER,
            ),
            transport_layer: DoCanTransportLayer::new(
                &this.transport_layer,
                busid::CAN_0,
                context,
                &CLASSIC_ADDRESSING_FILTER,
                &this.physical_transceiver,
                &this.tick_generator,
                &TRANSPORT_LAYER_CONFIG,
                &DOCAN,
            ),
            layers: [&this.transport_layer],
            transport_layers: DoCanTransportLayerContainer::new(&this.layers),
            tick_generator: TickGeneratorRunnableAdapter {
                this: &this.tick_generator,
                layers: &this.transport_layers,
                tick_timeout: Timeout::new(),
                context,
                node: QueueNode::new(),
            },
            node: QueueNode::new(),
        }
    }
}

impl LifecycleComponent for DoCanSystem {
    fn base(&self) -> &ComponentBase {
        &self.base
    }

    fn init(&'static self) {
        CLASSIC_ADDRESSING_FILTER.init(&ADDRESSES, &CODECS);
        self.transition_done();
    }

    fn run(&'static self) {
        for layer in self.transport_layers.transport_layers() {
            self.transport_system.add_transport_layer(*layer);
        }
        self.transport_layers.init();
        openbsw_async::schedule_at_fixed_rate(
            self.context,
            self,
            &self.cyclic_timeout,
            TIMEOUT_DOCAN_SYSTEM,
            TimeUnit::Milliseconds,
        );
        self.transition_done();
    }

    fn shutdown(&'static self) {
        self.cyclic_timeout.cancel();
        for layer in self.transport_layers.transport_layers() {
            self.transport_system.remove_transport_layer(*layer);
        }
        self.transition_done();
    }
}

impl Runnable for DoCanSystem {
    fn execute(&self) {
        self.transport_layers.cyclic_task(system_time_us32());
    }

    fn node(&self) -> &QueueNode<dyn Runnable> {
        &self.node
    }
}
