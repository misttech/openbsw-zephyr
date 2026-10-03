// Copyright 2026 Mist Tecnologia LTDA. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

//! The application's configuration, ported from `openbswConfig`'s `async/Config.h`, the
//! thread names of `main.cpp`, `app/appConfig.h`, `busid/BusId.h` and
//! `transport/TransportConfiguration.h`.

use core::ffi::CStr;

use openbsw_async::ContextType;

/// The highest-priority task.
pub const TASK_SYSADMIN: ContextType = 0;
/// The CAN task.
pub const TASK_CAN: ContextType = 1;
/// The demo task.
pub const TASK_DEMO: ContextType = 2;
/// The UDS task.
pub const TASK_UDS: ContextType = 3;
/// The lowest-priority task.
pub const TASK_BACKGROUND: ContextType = 4;
/// The number of tasks.
pub const TASK_COUNT: usize = 5;

/// The task names, in context order.
pub const TASK_NAMES: [&CStr; TASK_COUNT] = [c"sysadmin", c"can", c"demo", c"uds", c"background"];

/// The number of interrupt groups (`ISR_GROUP_COUNT`: test, can, ethernet).
pub const ISR_GROUP_COUNT: usize = 3;
/// The interrupt group names, as `main.cpp` names them: "ethernet" only with
/// `PLATFORM_SUPPORT_ETHERNET`, which the demo's CMake file defines for
/// `CONFIG_NETWORKING`. This application has no networking, so the ethernet group is
/// unnamed and `stats cpu` leaves it out, as the C++ does.
pub const ISR_GROUP_NAMES: [Option<&[u8]>; ISR_GROUP_COUNT] = [Some(b"test"), Some(b"can"), None];

/// The bus ids, from `busid/BusId.h`.
pub mod busid {
    /// The diagnostics bus.
    pub const SELFDIAG: u8 = 1;
    /// The CAN bus.
    pub const CAN_0: u8 = 2;

    /// The name of a bus, as `BusIdTraits::getName`.
    pub const fn name(index: u8) -> &'static [u8] {
        match index {
            SELFDIAG => b"SELFDIAG",
            CAN_0 => b"CAN_0",
            _ => b"INVALID",
        }
    }
}

/// This ECU's diagnostic address (`app/appConfig.h`).
pub const LOGICAL_ADDRESS: u16 = 0x002A;

/// The transport addresses and sizes of `TransportConfiguration.h`.
pub mod transport_configuration {
    /// The functional address every ECU answers (`FUNCTIONAL_ALL_ISO14229`).
    pub const FUNCTIONAL_ALL_ISO14229: u16 = 0x00DF;
    /// The largest functional request (`MAX_FUNCTIONAL_MESSAGE_PAYLOAD_SIZE`).
    pub const MAX_FUNCTIONAL_MESSAGE_PAYLOAD_SIZE: u16 = 6;
    /// The largest diagnostic message (`DIAG_PAYLOAD_SIZE`).
    pub const DIAG_PAYLOAD_SIZE: u16 = 4095;
}
