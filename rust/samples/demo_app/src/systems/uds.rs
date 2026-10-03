// Copyright 2026 Mist Tecnologia LTDA. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

//! The UDS system, ported from `systems/UdsSystem.cpp` and the demo's `udsConfiguration`:
//! the diagnostic dispatcher on the SELFDIAG bus with the demo's services and the two data
//! identifiers it answers, registered with the transport system at its init level.

use openbsw_async::{ContextType, QueueNode, Runnable, TimeUnit, Timeout};
use openbsw_async_zephyr::LockType;
use openbsw_lifecycle::{ComponentBase, LifecycleComponent};
use openbsw_transport::{ShutdownListener, TransportLayer};
use openbsw_uds::jobs::{DataIdentifierJob, ReadIdentifierFromMemory};
use openbsw_uds::services::{
    CommunicationControl, DiagnosticSessionControl, ReadDataByIdentifier, RequestRoutineResults,
    RoutineControl, StartRoutine, StopRoutine, TesterPresent, WriteDataByIdentifier,
};
use openbsw_uds::{
    DiagDispatcher, DiagJob, DiagJobRoot, DiagReturnCode, DiagSessionMask, DiagnosisConfiguration,
    IncomingDiagConnection, JobBase, Request, SessionPersistence, ShutdownType,
    TransportConfiguration, UdsLifecycleConnector, set_default_diag_session_manager,
    set_diag_job_root,
};

use super::transport::TransportSystem;
use crate::config::{busid, transport_configuration};
use crate::logger::GLOBAL;

/// The response to ReadDataByIdentifier 0xF186 (`responseData_22f186`).
static RESPONSE_DATA_22F186: [u8; 24] = [
    0x01, 0x02, 0x00, 0x02, 0x22, 0x02, 0x16, 0x0F, 0x01, 0x00, 0x00, 0x6D, 0x2F, 0x00, 0x00, 0x01,
    0x06, 0x00, 0x00, 0x8F, 0xE0, 0x00, 0x00, 0x01,
];

/// The demo's `UdsLifecycleConnector`: every mode change is possible and accepted.
struct DemoLifecycleConnector;

impl UdsLifecycleConnector for DemoLifecycleConnector {
    fn is_mode_change_possible(&self) -> bool {
        true
    }

    fn request_powerdown(&self, _rapid: bool, _time: &mut u8) -> bool {
        true
    }

    fn request_shutdown(&self, _shutdown_type: ShutdownType, _timeout: u32) -> bool {
        true
    }
}

/// The demo's `DummySessionPersistence`: nothing is read or written.
struct DummySessionPersistence;

impl SessionPersistence for DummySessionPersistence {
    fn read_session(&self, _session_control: &'static DiagnosticSessionControl) {}

    fn write_session(&self, _session_control: &'static DiagnosticSessionControl, _session: u8) {}
}

/// ReadDataByIdentifier 0xF102, the potentiometer: the port of the demo's
/// `ReadIdentifierPot`, which answers a fixed reading on a board without the input.
struct ReadIdentifierPot {
    job: DataIdentifierJob,
}

impl ReadIdentifierPot {
    const fn new() -> Self {
        Self { job: DataIdentifierJob::new(&[0x22, 0xF1, 0x02], DiagSessionMask::ALL_SESSIONS) }
    }
}

impl DiagJob for ReadIdentifierPot {
    fn base(&self) -> &JobBase {
        self.job.base()
    }

    fn verify(&self, request: Request<'_>) -> DiagReturnCode {
        self.job.verify(request)
    }

    fn process(
        &'static self,
        connection: &'static IncomingDiagConnection,
        _request: Request<'_>,
    ) -> DiagReturnCode {
        let response = connection.release_request_get_response();
        let adc_value: u32 = 0x1234_5678;
        let _ = response.append_data(&adc_value.to_be_bytes());
        let _ = connection.send_positive_response_internal(response.length() as u16, self);
        DiagReturnCode::Ok
    }
}

/// Told when the dispatcher has shut down (`UdsSystem::shutdownComplete`).
struct ShutdownComplete {
    owner: &'static UdsSystem,
}

impl ShutdownListener for ShutdownComplete {
    fn shutdown_done(&self, _layer: &'static dyn TransportLayer) {
        self.owner.timeout.cancel();
        self.owner.transition_done();
    }
}

/// The UDS component.
pub struct UdsSystem {
    base: ComponentBase,
    context: ContextType,
    transport_system: &'static TransportSystem,
    lifecycle_connector: DemoLifecycleConnector,
    persistence: DummySessionPersistence,
    job_root: DiagJobRoot,
    diagnostic_session_control: DiagnosticSessionControl,
    communication_control: CommunicationControl,
    configuration: DiagnosisConfiguration<5, 1, 16>,
    dispatcher: DiagDispatcher<LockType>,
    read_data_by_identifier: ReadDataByIdentifier,
    write_data_by_identifier: WriteDataByIdentifier,
    routine_control: RoutineControl,
    start_routine: StartRoutine,
    stop_routine: StopRoutine,
    request_routine_results: RequestRoutineResults,
    read_22f186: ReadIdentifierFromMemory,
    read_22f102: ReadIdentifierPot,
    tester_present: TesterPresent,
    shutdown_complete: ShutdownComplete,
    timeout: Timeout,
    node: QueueNode<dyn Runnable>,
}

impl UdsSystem {
    /// A UDS system whose transitions and dispatcher run on `context`, answering as
    /// `uds_address` through `transport_system`, given its own `'static` address.
    pub const fn new(
        this: &'static Self,
        transport_system: &'static TransportSystem,
        context: ContextType,
        uds_address: u16,
    ) -> Self {
        Self {
            base: ComponentBase::with_context(context),
            context,
            transport_system,
            lifecycle_connector: DemoLifecycleConnector,
            persistence: DummySessionPersistence,
            job_root: DiagJobRoot::new(),
            diagnostic_session_control: DiagnosticSessionControl::new(
                &this.lifecycle_connector,
                context,
                &this.persistence,
            ),
            communication_control: CommunicationControl::with_default_mask(),
            configuration: DiagnosisConfiguration::new(
                uds_address,
                transport_configuration::FUNCTIONAL_ALL_ISO14229,
                busid::SELFDIAG,
                transport_configuration::DIAG_PAYLOAD_SIZE,
                true,  // activate outgoing and pending
                false, // accept all requests
                true,  // copy functional requests
                context,
                TransportConfiguration::DEMO,
            ),
            dispatcher: DiagDispatcher::new(
                &this.dispatcher,
                &this.configuration,
                busid::SELFDIAG,
                &this.diagnostic_session_control,
                &this.job_root,
                &GLOBAL,
            ),
            read_data_by_identifier: ReadDataByIdentifier::new(),
            write_data_by_identifier: WriteDataByIdentifier::new(),
            routine_control: RoutineControl::new(),
            start_routine: StartRoutine::new(),
            stop_routine: StopRoutine::new(),
            request_routine_results: RequestRoutineResults::new(),
            read_22f186: ReadIdentifierFromMemory::new(
                0xF186,
                &RESPONSE_DATA_22F186,
                DiagSessionMask::ALL_SESSIONS,
            ),
            read_22f102: ReadIdentifierPot::new(),
            tester_present: TesterPresent::new(),
            shutdown_complete: ShutdownComplete { owner: this },
            timeout: Timeout::new(),
            node: QueueNode::new(),
        }
    }

    /// The jobs, in the order `addDiagJobs` adds them.
    fn jobs(&'static self) -> [&'static dyn DiagJob; 11] {
        [
            // 22 - ReadDataByIdentifier
            &self.read_data_by_identifier,
            &self.read_22f186,
            &self.read_22f102,
            // 2E - WriteDataByIdentifier
            &self.write_data_by_identifier,
            // 31 - Routine Control
            &self.routine_control,
            &self.start_routine,
            &self.stop_routine,
            &self.request_routine_results,
            // Services
            &self.tester_present,
            &self.diagnostic_session_control,
            &self.communication_control,
        ]
    }
}

impl LifecycleComponent for UdsSystem {
    fn base(&self) -> &ComponentBase {
        &self.base
    }

    fn init(&'static self) {
        // The timeout manager needs a context in which to run asynchronous calls.
        let _ = self.dispatcher.init();
        set_default_diag_session_manager(Some(&self.diagnostic_session_control));
        // The C++ root registers itself when constructed.
        set_diag_job_root(Some(&self.job_root));
        self.diagnostic_session_control.set_diag_dispatcher(Some(&self.dispatcher));
        self.transport_system.add_transport_layer(&self.dispatcher);
        for job in self.jobs() {
            let _ = self.dispatcher.add_abstract_diag_job(job);
        }
        self.transition_done();
    }

    fn run(&'static self) {
        openbsw_async::schedule_at_fixed_rate(
            self.context,
            self,
            &self.timeout,
            10,
            TimeUnit::Milliseconds,
        );
        self.transition_done();
    }

    fn shutdown(&'static self) {
        for job in self.jobs() {
            self.dispatcher.remove_abstract_diag_job(job);
        }
        self.diagnostic_session_control.set_diag_dispatcher(None);
        self.diagnostic_session_control.shutdown();
        self.transport_system.remove_transport_layer(&self.dispatcher);
        let _ = self.dispatcher.shutdown(&self.shutdown_complete);
    }
}

impl Runnable for UdsSystem {
    fn execute(&self) {}

    fn node(&self) -> &QueueNode<dyn Runnable> {
        &self.node
    }
}
