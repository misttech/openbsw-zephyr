// Copyright 2026 Mist Tecnologia LTDA. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

//! The demo's console, ported from `src/console/console.cpp`: line input from the standard
//! streams, tagged output, and the command tree the systems register their commands in.

use core::sync::atomic::{AtomicBool, Ordering};

use openbsw_bsp_zephyr::ZephyrStdio;
use openbsw_console::{AsyncConsole, StdioConsoleInput};

static STDIO: ZephyrStdio = ZephyrStdio;
static STDIO_CONSOLE_INPUT: StdioConsoleInput = StdioConsoleInput::new(&STDIO, b" ", b"\r\n");
/// The command tree.
pub static ASYNC_CONSOLE: AsyncConsole = AsyncConsole::new(&ASYNC_CONSOLE);
static ENABLE_STDIO_CONSOLE: AtomicBool = AtomicBool::new(false);

/// Read the standard input on `run`.
pub fn enable() {
    ENABLE_STDIO_CONSOLE.store(true, Ordering::Relaxed);
}

/// Stop reading the standard input.
#[expect(
    dead_code,
    reason = "part of the console.h interface; the demo never disables its console"
)]
pub fn disable() {
    ENABLE_STDIO_CONSOLE.store(false, Ordering::Relaxed);
}

/// Connect the input to the command tree.
pub fn init() {
    ASYNC_CONSOLE.init();
    STDIO_CONSOLE_INPUT.init(&ASYNC_CONSOLE);
}

/// Read one byte of input, if enabled.
pub fn run() {
    if ENABLE_STDIO_CONSOLE.load(Ordering::Relaxed) {
        STDIO_CONSOLE_INPUT.run();
    }
}
