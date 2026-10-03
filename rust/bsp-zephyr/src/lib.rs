// Copyright 2026 Mist Tecnologia LTDA. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

//! Port of `libs/bspZephyr`: what OpenBSW's board support asks of Zephyr.
//!
//! - [`system_timer`]: the clocks (`SystemTimer.cpp`).
//! - [`ZephyrStdio`]: the console bytes (`Stdio.cpp`).
//!
//! The CAN transceiver follows with the `cpp2can` crate.

#![no_std]
#![deny(unsafe_op_in_unsafe_fn)]
#![warn(missing_docs)]

pub mod system_timer;

use openbsw_util::stream::Stdio;

/// The console through the shim's UART functions: the port of `Stdio.cpp`.
pub struct ZephyrStdio;

impl Stdio for ZephyrStdio {
    /// The next console byte, or 255 when none is waiting (the C++ stores `-1` in an
    /// `unsigned char`).
    fn get_byte(&self) -> i32 {
        zephyr_ffi::uart_poll_in()
    }

    fn put_byte(&self, byte: u8) {
        zephyr_ffi::uart_poll_out(byte);
    }
}

#[cfg(test)]
mod tests {
    extern crate std;

    use std::sync::Mutex;
    use std::vec::Vec;

    use zephyr_ffi::host::Hooks;

    use super::*;

    /// The hooks are process-wide, so the tests run one at a time.
    static SERIAL: Mutex<()> = Mutex::new(());

    /// A 170 MHz clock two seconds and one cycle after boot, and a console that keeps what
    /// it is given and hands out `A` once.
    struct Board {
        written: Mutex<Vec<u8>>,
        input: Mutex<Vec<i32>>,
    }

    impl Hooks for Board {
        fn cycle_get_64(&self) -> u64 {
            340_000_001
        }
        fn cycles_per_sec(&self) -> u32 {
            170_000_000
        }
        fn uart_poll_in(&self) -> i32 {
            self.input.lock().unwrap().pop().unwrap_or(zephyr_ffi::UART_NO_BYTE)
        }
        fn uart_poll_out(&self, byte: u8) {
            self.written.lock().unwrap().push(byte);
        }
    }

    static BOARD: Board = Board { written: Mutex::new(Vec::new()), input: Mutex::new(Vec::new()) };

    #[test]
    fn the_clocks_convert_the_cycle_counter() {
        let _serial = SERIAL.lock().unwrap();
        zephyr_ffi::host::install(&BOARD);
        assert_eq!(system_timer::system_time_us32(), 2_000_000);
        assert_eq!(system_timer::system_time_ms32(), 2_000);
        assert_eq!(system_timer::system_time_ns(), 2_000_000_005);
        assert_eq!(system_timer::system_ticks32(), 340_000_001);
        assert_eq!(system_timer::ticks_to_ns(170), 1_000);
        assert_eq!(system_timer::ticks_to_us(170_000), 1_000);
        // Two hours of cycles: past 2^32 microseconds, which a 32-bit result wraps.
        assert_eq!(system_timer::ticks_to_us(170_000_000 * 7_200), 7_200_000_000);
    }

    #[test]
    fn stdio_is_the_console() {
        let _serial = SERIAL.lock().unwrap();
        zephyr_ffi::host::install(&BOARD);
        BOARD.input.lock().unwrap().push(i32::from(b'A'));
        assert_eq!(ZephyrStdio.get_byte(), i32::from(b'A'));
        assert_eq!(ZephyrStdio.get_byte(), 255);
        ZephyrStdio.put_byte(b'o');
        ZephyrStdio.put_byte(b'k');
        assert_eq!(core::mem::take(&mut *BOARD.written.lock().unwrap()), b"ok");
    }
}
