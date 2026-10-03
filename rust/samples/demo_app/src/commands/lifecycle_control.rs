// Copyright 2026 Mist Tecnologia LTDA. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

//! The `lc` command, ported from `lifecycle/console/LifecycleControlCommand.cpp`: reboot
//! and power off through lifecycle level 0, and the fault provocations (no-ops here, as in
//! the Zephyr demo).

use openbsw_util::command::{CommandContext, CommandInfo, GroupCommand};

use crate::LIFECYCLE_MANAGER;

const ID_REBOOT: u8 = 0;
const ID_POWEROFF: u8 = 1;
const ID_UDEF: u8 = 2;
const ID_PABT: u8 = 3;
const ID_DABT: u8 = 4;
const ID_ASSERT: u8 = 5;

static INFO: [CommandInfo; 7] = [
    CommandInfo::new(b"lc", b"lifecycle command", 0),
    CommandInfo::new(b"reboot", b"reboot the system", ID_REBOOT),
    CommandInfo::new(b"poweroff", b"poweroff the system", ID_POWEROFF),
    CommandInfo::new(b"udef", b"forces an undefined instruction exception", ID_UDEF),
    CommandInfo::new(b"pabt", b"forces a prefetch abort exception", ID_PABT),
    CommandInfo::new(b"dabt", b"forces a data abort exception", ID_DABT),
    CommandInfo::new(b"assert", b"forces an assert", ID_ASSERT),
];

/// The `lc` command.
pub static LIFECYCLE_CONTROL_COMMAND: GroupCommand = GroupCommand::new(&INFO, &execute);

fn execute(_context: &mut CommandContext<'_, '_>, idx: u8) {
    match idx {
        ID_REBOOT | ID_POWEROFF => LIFECYCLE_MANAGER.transition_to_level(0),
        ID_UDEF | ID_PABT | ID_DABT => {}
        // estd_assert(false) in C++: stop the firmware.
        ID_ASSERT => panic!("requested assert"),
        _ => {}
    }
}
