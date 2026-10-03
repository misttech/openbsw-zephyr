// Copyright 2026 Mist Tecnologia LTDA. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

//! OpenBSW's `demo_app` in Rust: the port of `samples/demo_app`.
//!
//! This is the skeleton: it boots, prints a line through the console UART, and
//! sleeps. The systems follow, one crate at a time.

#![no_std]
#![deny(unsafe_op_in_unsafe_fn)]

use openbsw_util::format::StringWriter;
use openbsw_util::stream::{Stdio, StdoutStream};

/// The console, through the shim's UART functions.
struct Console;

impl Stdio for Console {
    fn get_byte(&self) -> i32 {
        zephyr_ffi::uart_poll_in()
    }

    fn put_byte(&self, byte: u8) {
        zephyr_ffi::uart_poll_out(byte);
    }
}

static CONSOLE: Console = Console;

/// The application's entry point, called by `rust_main.c` from Zephyr's `main`.
#[unsafe(no_mangle)]
pub extern "C" fn rust_main() {
    let mut stdout = StdoutStream::new(&CONSOLE);
    StringWriter::new(&mut stdout).printf(b"OpenBSW demo_app in Rust: %s\r\n", &["hello".into()]);
    loop {
        zephyr_ffi::msleep(1000);
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
