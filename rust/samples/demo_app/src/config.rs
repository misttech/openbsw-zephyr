// Copyright 2026 Mist Tecnologia LTDA. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

//! The application's task configuration, ported from `openbswConfig`'s `async/Config.h`
//! and the thread names of `main.cpp`. The bus ids and the UDS address follow with the
//! systems that use them.

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
