// Copyright 2026 Mist Tecnologia LTDA. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

//! OpenBSW's `demo_app` in Rust: the port of `samples/demo_app/src/main.cpp`.
//!
//! The lifecycle manager brings the systems up one run level at a time on their async
//! contexts, which are Zephyr threads; the CAN system opens the bus and the demo system
//! sends a frame a second; a 1000 ms runnable logs the context it runs on, and a 10 ms
//! runnable drains the log onto the console.

#![no_std]
#![deny(unsafe_op_in_unsafe_fn)]

mod commands;
mod config;
mod console;
mod logger;
mod runtime_monitor;
mod systems;

use openbsw_async::{QueueNode, Runnable, TimeUnit, Timeout};
use openbsw_async_zephyr::{LockType, TaskContext, ZephyrAdapter};
use openbsw_bsp_zephyr::ZephyrCanTransceiver;
use openbsw_bsp_zephyr::system_timer::system_time_us32;
use openbsw_console::AsyncCommandWrapper;
use openbsw_lifecycle::{LIFECYCLE, LifecycleListener, LifecycleManager, ListenerNode, Transition};
use openbsw_util::log_debug;
use openbsw_util::log_info;

use commands::lifecycle_control::LIFECYCLE_CONTROL_COMMAND;
use commands::statistics::STATISTICS_COMMAND;
use config::{
    LOGICAL_ADDRESS, TASK_BACKGROUND, TASK_CAN, TASK_COUNT, TASK_DEMO, TASK_NAMES, TASK_SYSADMIN,
    TASK_UDS, busid,
};
use console::ASYNC_CONSOLE;
use logger::DEMO;
use runtime_monitor::RUNTIME_MONITOR;
use systems::can::CanSystem;
use systems::demo::DemoSystem;
use systems::docan::DoCanSystem;
use systems::runtime::RuntimeSystem;
use systems::sysadmin::SysAdminSystem;
use systems::transport::TransportSystem;
use systems::uds::UdsSystem;

const MAX_NUM_COMPONENTS: usize = 16;
const MAX_NUM_LEVELS: usize = 8;
const MAX_NUM_COMPONENTS_PER_LEVEL: usize = MAX_NUM_COMPONENTS;

static CONTEXTS: [TaskContext; TASK_COUNT] = [
    TaskContext::new(&CONTEXTS[0]),
    TaskContext::new(&CONTEXTS[1]),
    TaskContext::new(&CONTEXTS[2]),
    TaskContext::new(&CONTEXTS[3]),
    TaskContext::new(&CONTEXTS[4]),
];
static ASYNC_ADAPTER: ZephyrAdapter<TASK_COUNT> = ZephyrAdapter::new(&CONTEXTS, TASK_NAMES);

static LIFECYCLE_MANAGER: LifecycleManager<
    MAX_NUM_COMPONENTS,
    MAX_NUM_LEVELS,
    MAX_NUM_COMPONENTS_PER_LEVEL,
    LockType,
> = LifecycleManager::new(TASK_SYSADMIN, &system_time_us32);

static RUNTIME_SYSTEM: RuntimeSystem = RuntimeSystem::new(TASK_BACKGROUND, &STATISTICS_COMMAND);
/// `_asyncCommandWrapper_for_statisticsCommand`: `stats` runs on the background context.
static STATISTICS_WRAPPER: AsyncCommandWrapper = AsyncCommandWrapper::new(
    &STATISTICS_WRAPPER,
    &ASYNC_CONSOLE,
    STATISTICS_COMMAND.command(),
    TASK_BACKGROUND,
);
/// `_asyncCommandWrapper_for_lifecycleControlCommand`: `lc` runs on the sysadmin context.
static LIFECYCLE_CONTROL_WRAPPER: AsyncCommandWrapper = AsyncCommandWrapper::new(
    &LIFECYCLE_CONTROL_WRAPPER,
    &ASYNC_CONSOLE,
    &LIFECYCLE_CONTROL_COMMAND,
    TASK_SYSADMIN,
);
static SYS_ADMIN_SYSTEM: SysAdminSystem = SysAdminSystem::new(TASK_SYSADMIN);
static CAN_TRANSCEIVER0: ZephyrCanTransceiver =
    ZephyrCanTransceiver::new(&CAN_TRANSCEIVER0, TASK_CAN, busid::CAN_0, busid::name(busid::CAN_0));
static CAN_SYSTEM: CanSystem = CanSystem::new(TASK_CAN, &CAN_TRANSCEIVER0);
static TRANSPORT_SYSTEM: TransportSystem = TransportSystem::new(&TRANSPORT_SYSTEM, TASK_UDS);
static DOCAN_SYSTEM: DoCanSystem =
    DoCanSystem::new(&DOCAN_SYSTEM, &TRANSPORT_SYSTEM, &CAN_TRANSCEIVER0, TASK_CAN);
static UDS_SYSTEM: UdsSystem =
    UdsSystem::new(&UDS_SYSTEM, &TRANSPORT_SYSTEM, TASK_UDS, LOGICAL_ADDRESS);
static DEMO_SYSTEM: DemoSystem = DemoSystem::new(TASK_DEMO, &CAN_SYSTEM);

/// Remembers when level 0 is reached, so `main` can reset.
struct LifecycleMonitor {
    ready_for_reset: core::sync::atomic::AtomicBool,
    node: ListenerNode,
}

impl LifecycleListener for LifecycleMonitor {
    fn lifecycle_level_reached(&self, level: u8, _transition: Transition) {
        if level == 0 {
            self.ready_for_reset.store(true, core::sync::atomic::Ordering::Relaxed);
        }
    }

    fn node(&self) -> &ListenerNode {
        &self.node
    }
}

static LIFECYCLE_MONITOR: LifecycleMonitor = LifecycleMonitor {
    ready_for_reset: core::sync::atomic::AtomicBool::new(false),
    node: ListenerNode::new(),
};

/// Logs the context it runs on; from the sysadmin context it re-executes itself on the
/// demo context.
struct DemoRunnable(QueueNode<dyn Runnable>);

impl Runnable for DemoRunnable {
    fn execute(&self) {
        let context = openbsw_async::current_context();
        log_debug!(
            DEMO,
            b"demo ctx: %d name: %s",
            context,
            openbsw_async::binding::task_name(context)
        );
        if context == TASK_SYSADMIN {
            openbsw_async::execute(TASK_DEMO, &DEMO_RUNNABLE);
        }
    }

    fn node(&self) -> &QueueNode<dyn Runnable> {
        &self.0
    }
}

/// Drains one log entry and one console byte.
struct DemoRunnable2(QueueNode<dyn Runnable>);

impl Runnable for DemoRunnable2 {
    fn execute(&self) {
        logger::run();
        console::run();
    }

    fn node(&self) -> &QueueNode<dyn Runnable> {
        &self.0
    }
}

static DEMO_RUNNABLE: DemoRunnable = DemoRunnable(QueueNode::new());
static DEMO_RUNNABLE2: DemoRunnable2 = DemoRunnable2(QueueNode::new());
static TIMEOUT: Timeout = Timeout::new();
static TIMEOUT2: Timeout = Timeout::new();

fn static_shutdown() -> ! {
    log_info!(LIFECYCLE, b"Lifecycle shutdown complete");
    logger::flush();
    zephyr_ffi::reboot_cold()
}

/// The application's entry point, called by `rust_main.c` from Zephyr's `main`.
#[unsafe(no_mangle)]
pub extern "C" fn rust_main() {
    logger::init();
    console::init();
    console::enable();
    // The C++ wrappers register themselves when constructed; here the systems' commands
    // are registered next to them.
    ASYNC_CONSOLE.add_command(&STATISTICS_WRAPPER);
    ASYNC_CONSOLE.add_command(&LIFECYCLE_CONTROL_WRAPPER);
    runtime_monitor::init();
    // The C++ `BusIdTraits::getName` is a global; the transport crate asks for it.
    openbsw_transport::set_bus_name_resolver(&busid::name);
    openbsw_async::set_binding(&ASYNC_ADAPTER);
    ASYNC_ADAPTER.init();
    LIFECYCLE_MANAGER.add_lifecycle_listener(&LIFECYCLE_MONITOR);
    LIFECYCLE_MANAGER.add_component(b"runtime", &RUNTIME_SYSTEM, 1);
    LIFECYCLE_MANAGER.add_component(b"can", &CAN_SYSTEM, 2);
    LIFECYCLE_MANAGER.add_component(b"transport", &TRANSPORT_SYSTEM, 4);
    LIFECYCLE_MANAGER.add_component(b"docan", &DOCAN_SYSTEM, 5);
    LIFECYCLE_MANAGER.add_component(b"uds", &UDS_SYSTEM, 6);
    LIFECYCLE_MANAGER.add_component(b"sysadmin", &SYS_ADMIN_SYSTEM, 7);
    LIFECYCLE_MANAGER.add_component(b"demo", &DEMO_SYSTEM, 8);
    LIFECYCLE_MANAGER.transition_to_level(MAX_NUM_LEVELS as u8);
    RUNTIME_MONITOR.start();
    ASYNC_ADAPTER.run();
    openbsw_async::schedule_at_fixed_rate(
        TASK_SYSADMIN,
        &DEMO_RUNNABLE,
        &TIMEOUT,
        1000,
        TimeUnit::Milliseconds,
    );
    openbsw_async::schedule_at_fixed_rate(
        TASK_DEMO,
        &DEMO_RUNNABLE2,
        &TIMEOUT2,
        10,
        TimeUnit::Milliseconds,
    );
    loop {
        zephyr_ffi::msleep(1000);
        if LIFECYCLE_MONITOR.ready_for_reset.load(core::sync::atomic::Ordering::Relaxed) {
            static_shutdown();
        }
    }
}

#[cfg(target_os = "none")]
#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    unsafe extern "C" {
        fn rust_panic_wrap() -> !;
    }
    // SAFETY: `rust_panic_wrap` is the C function in rust_main.c that calls k_panic().
    unsafe { rust_panic_wrap() }
}
