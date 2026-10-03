// Copyright 2026 Mist Tecnologia LTDA. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

//! The `stats` command, ported from `lifecycle/console/StatisticsCommand.cpp`: CPU time per
//! task and interrupt group from the last one-second snapshot, and the stack usage.

use core::cell::Cell;

use openbsw_async::{Async, Lock};
use openbsw_async_zephyr::LockType;
use openbsw_runtime::{
    Mode, RuntimeStatistics, StatisticsContainer, StatisticsWriter, TickConversion,
};
use openbsw_util::command::{CommandContext, CommandInfo, GroupCommand};
use openbsw_util::format::{Arg, with_shared_writer};

use crate::ASYNC_ADAPTER;
use crate::config::{ISR_GROUP_COUNT, TASK_COUNT};
use crate::runtime_monitor::{RUNTIME_MONITOR, RuntimeMonitorType};

const ID_CPU: u8 = 0;
const ID_STACK: u8 = 1;
const ID_ALL: u8 = 2;

static INFO: [CommandInfo; 4] = [
    CommandInfo::new(b"stats", b"lifecycle statistics command", 0),
    CommandInfo::new(b"cpu", b"prints CPU statistics", ID_CPU),
    CommandInfo::new(b"stack", b"prints stack statistics", ID_STACK),
    CommandInfo::new(b"all", b"prints all statistics", ID_ALL),
];

static TASK_SNAPSHOT: [RuntimeStatistics; TASK_COUNT] =
    [const { RuntimeStatistics::new() }; TASK_COUNT];
static ISR_GROUP_SNAPSHOT: [RuntimeStatistics; ISR_GROUP_COUNT] =
    [const { RuntimeStatistics::new() }; ISR_GROUP_COUNT];

/// The `stats` command and the snapshot it prints from.
pub struct StatisticsCommand {
    runtime_monitor: &'static RuntimeMonitorType,
    task_statistics: StatisticsContainer<RuntimeStatistics>,
    isr_group_statistics: StatisticsContainer<RuntimeStatistics>,
    ticks_per_us: Cell<Option<u32>>,
    total_runtime: Cell<u32>,
    command: GroupCommand,
}

// SAFETY: the snapshot is taken under the platform lock on the background context, and
// the command reads it on the same context.
unsafe impl Sync for StatisticsCommand {}

/// The one `stats` command.
pub static STATISTICS_COMMAND: StatisticsCommand = StatisticsCommand {
    runtime_monitor: &RUNTIME_MONITOR,
    task_statistics: StatisticsContainer::new(&TASK_SNAPSHOT, None),
    isr_group_statistics: StatisticsContainer::new(&ISR_GROUP_SNAPSHOT, None),
    ticks_per_us: Cell::new(None),
    total_runtime: Cell::new(0),
    command: GroupCommand::new(&INFO, &execute),
};

fn execute(context: &mut CommandContext<'_, '_>, idx: u8) {
    STATISTICS_COMMAND.execute_command(context, idx);
}

fn format(writer: &mut StatisticsWriter<'_, '_, '_>, statistics: &RuntimeStatistics) {
    writer.write_runtime_percentage(b"%", statistics.total_runtime());
    writer.write_runtime_ms(b"total ", 9, statistics.total_runtime());
    writer.write_number(b"runs ", 6, statistics.total_run_count());
    writer.write_runtime(b"avg ", 6, statistics.average_runtime());
    writer.write_runtime(b"min ", 6, statistics.min_runtime());
    writer.write_runtime(b"max ", 6, statistics.max_runtime());
}

impl StatisticsCommand {
    /// The command to register with the console.
    pub const fn command(&'static self) -> &'static GroupCommand {
        &self.command
    }

    /// How many clock ticks make a microsecond; unknown until set.
    pub fn set_ticks_per_us(&self, ticks_per_us: u32) {
        self.ticks_per_us.set(Some(ticks_per_us));
    }

    /// Take the snapshot the command prints from, and reset the monitor.
    pub fn cyclic_1000ms(&self) {
        let _lock = LockType::lock();
        self.task_statistics.copy_from(self.runtime_monitor.task_statistics());
        self.isr_group_statistics.copy_from(self.runtime_monitor.isr_group_statistics());
        self.total_runtime.set(self.runtime_monitor.reset());
    }

    fn execute_command(&self, context: &mut CommandContext<'_, '_>, idx: u8) {
        match idx {
            ID_CPU => self.print_cpu(context),
            ID_STACK => Self::print_stack(context),
            ID_ALL => {
                self.print_cpu(context);
                Self::print_stack(context);
            }
            _ => {}
        }
    }

    fn print_cpu(&self, context: &mut CommandContext<'_, '_>) {
        with_shared_writer(context, |writer| {
            let Some(ticks_per_us) = self.ticks_per_us.get() else {
                writer.printf(b"cannot print CPU statistics, ticksPerUs is unknown\n", &[]);
                return;
            };
            let total_runtime = self.total_runtime.get();
            let ticks = TickConversion::TicksPerUs(ticks_per_us);
            {
                let mut statistics_writer = StatisticsWriter::new(writer, total_runtime, ticks);
                statistics_writer.format_statistics_group(
                    &format,
                    b"task",
                    15,
                    self.task_statistics.iter(),
                );
                statistics_writer.write_eol();
                statistics_writer.format_statistics_group(
                    &format,
                    b"isr group",
                    15,
                    self.isr_group_statistics.iter(),
                );
                statistics_writer.write_eol();
            }
            writer.write(b"measurement time: ");
            // A fresh writer is at a line start, as the C++ one is after its writeEol().
            let mut statistics_writer = StatisticsWriter::new(writer, total_runtime, ticks);
            statistics_writer.set_mode(Mode::Value);
            statistics_writer.write_runtime(b"", 8, total_runtime);
        });
    }

    fn print_stack(context: &mut CommandContext<'_, '_>) {
        with_shared_writer(context, |writer| {
            for context in 0..TASK_COUNT as u8 {
                let name = ASYNC_ADAPTER.task_name(context);
                let (size, used) = ASYNC_ADAPTER.stack_usage(context).unwrap_or((0, 0));
                writer.printf(
                    b"stack:task=%s,size=%d,used=%d\n",
                    &[Arg::Str(Some(name)), size.into(), used.into()],
                );
            }
        });
    }
}
