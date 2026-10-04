// Copyright 2026 Mist Tecnologia LTDA. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

//! The run time monitor and its hook, the port of `AsyncBinding::RuntimeMonitorType`,
//! `ContextHookType` and the `runtimeMonitor`/`contextHook` objects of `main.cpp`.

use openbsw_async::Async;
use openbsw_async_zephyr::{ContextHook, LockType, set_context_hook};
use openbsw_bsp_zephyr::system_timer::system_ticks32;
use openbsw_runtime::{
    Clock, ContextEntry, RuntimeMonitor, RuntimeStatistics, StatisticsContainer,
};

use crate::ASYNC_ADAPTER;
use crate::config::{ISR_GROUP_COUNT, ISR_GROUP_NAMES, TASK_COUNT};

/// The monitor's clock, `getSystemTicks32Bit`.
pub struct SystemTicks;

impl Clock for SystemTicks {
    #[inline]
    fn ticks() -> u32 {
        system_ticks32()
    }
}

/// `AsyncBinding::RuntimeMonitorType`: run time statistics for contexts and functions alike.
pub type RuntimeMonitorType =
    RuntimeMonitor<RuntimeStatistics, RuntimeStatistics, LockType, SystemTicks>;
type Entry = ContextEntry<RuntimeStatistics, RuntimeStatistics>;

static TASK_ENTRIES: [Entry; TASK_COUNT] =
    [const { Entry::new(RuntimeStatistics::new()) }; TASK_COUNT];
static ISR_GROUP_ENTRIES: [Entry; ISR_GROUP_COUNT] =
    [const { Entry::new(RuntimeStatistics::new()) }; ISR_GROUP_COUNT];

fn task_name(idx: usize) -> Option<&'static [u8]> {
    u8::try_from(idx).ok().map(|context| ASYNC_ADAPTER.task_name(context))
}

fn isr_group_name(idx: usize) -> Option<&'static [u8]> {
    ISR_GROUP_NAMES.get(idx).copied().flatten()
}

static TASK_STATISTICS: StatisticsContainer<Entry> =
    StatisticsContainer::new(&TASK_ENTRIES, Some(&task_name));
static ISR_GROUP_STATISTICS: StatisticsContainer<Entry> =
    StatisticsContainer::new(&ISR_GROUP_ENTRIES, Some(&isr_group_name));
/// The monitor the tracing hooks feed.
pub static RUNTIME_MONITOR: RuntimeMonitorType =
    RuntimeMonitor::new(&TASK_STATISTICS, &ISR_GROUP_STATISTICS);

/// The port of `StaticContextHook<RuntimeMonitorType>`.
struct MonitorHook;

impl ContextHook for MonitorHook {
    fn enter_task(&self, task_idx: usize) {
        RUNTIME_MONITOR.enter_task(task_idx);
    }

    fn leave_task(&self, task_idx: usize) {
        RUNTIME_MONITOR.leave_task(task_idx);
    }

    fn enter_isr_group(&self, isr_group_idx: usize) {
        RUNTIME_MONITOR.enter_isr_group(isr_group_idx);
    }

    fn leave_isr_group(&self, isr_group_idx: usize) {
        RUNTIME_MONITOR.leave_isr_group(isr_group_idx);
    }
}

static MONITOR_HOOK: MonitorHook = MonitorHook;

/// Connect the tracing hooks to the monitor (the C++ `contextHook` object).
pub fn init() {
    set_context_hook(&MONITOR_HOOK);
}
