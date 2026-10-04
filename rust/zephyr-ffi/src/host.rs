// Copyright 2026 Mist Tecnologia LTDA. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

//! The host implementation: every call goes to the installed [`Hooks`].

extern crate std;

use core::ffi::{CStr, c_void};
use std::sync::RwLock;

use crate::{TaskEntry, UART_NO_BYTE, ZephyrResult};

/// What a test observes and answers in place of Zephyr. Every method has a neutral
/// default.
pub trait Hooks: Sync + Send {
    /// A thread was created.
    fn task_create(
        &self,
        _context: u32,
        _name: &'static CStr,
        _priority: i32,
        _entry: TaskEntry,
        _arg: *mut c_void,
    ) {
    }
    /// A thread was started.
    fn task_start(&self, _context: u32) {}
    /// The name of a thread.
    fn task_name(&self, _context: u32) -> &'static [u8] {
        b"<undefined>"
    }
    /// The stack size of a thread.
    fn task_stack_size(&self, _context: u32) -> u32 {
        0
    }
    /// The unused stack of a thread.
    fn task_stack_unused(&self, _context: u32) -> Option<u32> {
        None
    }
    /// Bits were posted to an event.
    fn event_post(&self, _context: u32, _mask: u32) {}
    /// A thread waits on an event; returns the bits set.
    fn event_wait(&self, _context: u32, mask: u32) -> u32 {
        mask
    }
    /// Bits were cleared on an event.
    fn event_clear(&self, _context: u32, _mask: u32) {}
    /// A timer was started.
    fn timer_start_us(&self, _context: u32, _microseconds: u32) {}
    /// A timer was stopped.
    fn timer_stop(&self, _context: u32) {}
    /// Interrupts were locked.
    fn irq_lock(&self) -> u32 {
        0
    }
    /// Interrupts were unlocked.
    fn irq_unlock(&self, _key: u32) {}
    /// Whether the caller is in an interrupt.
    fn is_in_isr(&self) -> bool {
        false
    }
    /// The current thread's priority.
    fn current_priority(&self) -> i32 {
        0
    }
    /// The scheduled thread's priority.
    fn sched_current_priority(&self) -> i32 {
        0
    }
    /// The thread sleeps.
    fn msleep(&self, _milliseconds: i32) {}
    /// The cycle counter.
    fn cycle_get_64(&self) -> u64 {
        0
    }
    /// The cycle counter's frequency.
    fn cycles_per_sec(&self) -> u32 {
        1_000_000
    }
    /// A console byte is read.
    fn uart_poll_in(&self) -> i32 {
        UART_NO_BYTE
    }
    /// A console byte is written.
    fn uart_poll_out(&self, _byte: u8) {}
    /// Whether the CAN device is ready.
    fn can_device_is_ready(&self) -> bool {
        true
    }
    /// Normal mode was requested.
    fn can_set_mode_normal(&self) -> ZephyrResult {
        0
    }
    /// The controller was started.
    fn can_start(&self) -> ZephyrResult {
        0
    }
    /// The controller was stopped.
    fn can_stop(&self) -> ZephyrResult {
        0
    }
    /// A match-all filter was added, with `user` for the receive callback; returns its id.
    fn can_add_rx_filter_all(&self, _user: *mut c_void) -> i32 {
        0
    }
    /// A filter was removed.
    fn can_remove_rx_filter(&self, _filter_id: i32) {}
    /// A frame was queued.
    fn can_send(
        &self,
        _id: u32,
        _extended: bool,
        _data: &[u8],
        _user: *mut c_void,
    ) -> ZephyrResult {
        0
    }
    /// The controller state.
    fn can_get_state(&self) -> Result<(i32, u8, u8), ZephyrResult> {
        Ok((0, 0, 0))
    }
    /// The bus bit rate.
    fn can_bitrate(&self) -> u32 {
        500_000
    }
    /// The LED PWM was set.
    fn pwm_set_led0(&self, _period_ns: u32, _pulse_ns: u32) -> ZephyrResult {
        0
    }
}

struct DefaultHooks;

impl Hooks for DefaultHooks {}

static DEFAULT_HOOKS: DefaultHooks = DefaultHooks;
static HOOKS: RwLock<&'static dyn Hooks> = RwLock::new(&DEFAULT_HOOKS);

/// Install `hooks` for every following call; tests that install hooks must not run in
/// parallel with each other.
pub fn install(hooks: &'static dyn Hooks) {
    *HOOKS.write().unwrap_or_else(|poisoned| poisoned.into_inner()) = hooks;
}

/// Reinstall the default hooks.
pub fn reset() {
    install(&DEFAULT_HOOKS);
}

fn hooks() -> &'static dyn Hooks {
    *HOOKS.read().unwrap_or_else(|poisoned| poisoned.into_inner())
}

pub(crate) fn task_create(
    context: u32,
    name: &'static CStr,
    priority: i32,
    entry: TaskEntry,
    arg: *mut c_void,
) {
    hooks().task_create(context, name, priority, entry, arg);
}

pub(crate) fn task_start(context: u32) {
    hooks().task_start(context);
}

pub(crate) fn task_name(context: u32) -> &'static [u8] {
    hooks().task_name(context)
}

pub(crate) fn task_stack_size(context: u32) -> u32 {
    hooks().task_stack_size(context)
}

pub(crate) fn task_stack_unused(context: u32) -> Option<u32> {
    hooks().task_stack_unused(context)
}

pub(crate) fn event_post(context: u32, mask: u32) {
    hooks().event_post(context, mask);
}

pub(crate) fn event_wait(context: u32, mask: u32) -> u32 {
    hooks().event_wait(context, mask)
}

pub(crate) fn event_clear(context: u32, mask: u32) {
    hooks().event_clear(context, mask);
}

pub(crate) fn timer_start_us(context: u32, microseconds: u32) {
    hooks().timer_start_us(context, microseconds);
}

pub(crate) fn timer_stop(context: u32) {
    hooks().timer_stop(context);
}

pub(crate) fn irq_lock() -> u32 {
    hooks().irq_lock()
}

pub(crate) fn irq_unlock(key: u32) {
    hooks().irq_unlock(key);
}

pub(crate) fn is_in_isr() -> bool {
    hooks().is_in_isr()
}

pub(crate) fn current_priority() -> i32 {
    hooks().current_priority()
}

pub(crate) fn sched_current_priority() -> i32 {
    hooks().sched_current_priority()
}

pub(crate) fn msleep(milliseconds: i32) {
    hooks().msleep(milliseconds);
}

pub(crate) fn reboot_cold() -> ! {
    panic!("reboot requested");
}

pub(crate) fn cycle_get_32() -> u32 {
    // The low half of the 64-bit count, which is what `k_cycle_get_32` returns.
    hooks().cycle_get_64() as u32
}

pub(crate) fn cycle_get_64() -> u64 {
    hooks().cycle_get_64()
}

pub(crate) fn cyc_to_us_floor32(cycles: u64) -> u32 {
    (cycles * 1_000_000 / u64::from(cycles_per_sec())) as u32
}

pub(crate) fn cyc_to_us_floor64(cycles: u64) -> u64 {
    cycles * 1_000_000 / u64::from(cycles_per_sec())
}

pub(crate) fn cyc_to_ms_floor32(cycles: u64) -> u32 {
    (cycles * 1_000 / u64::from(cycles_per_sec())) as u32
}

pub(crate) fn cyc_to_ns_floor64(cycles: u64) -> u64 {
    cycles * 1_000_000_000 / u64::from(cycles_per_sec())
}

pub(crate) fn cycles_per_sec() -> u32 {
    hooks().cycles_per_sec()
}

pub(crate) fn uart_poll_in() -> i32 {
    hooks().uart_poll_in()
}

pub(crate) fn uart_poll_out(byte: u8) {
    hooks().uart_poll_out(byte);
}

pub(crate) fn can_device_is_ready() -> bool {
    hooks().can_device_is_ready()
}

pub(crate) fn can_set_mode_normal() -> ZephyrResult {
    hooks().can_set_mode_normal()
}

pub(crate) fn can_start() -> ZephyrResult {
    hooks().can_start()
}

pub(crate) fn can_stop() -> ZephyrResult {
    hooks().can_stop()
}

pub(crate) fn can_add_rx_filter_all(user: *mut c_void) -> i32 {
    hooks().can_add_rx_filter_all(user)
}

pub(crate) fn can_remove_rx_filter(filter_id: i32) {
    hooks().can_remove_rx_filter(filter_id);
}

pub(crate) fn can_send(id: u32, extended: bool, data: &[u8], user: *mut c_void) -> ZephyrResult {
    hooks().can_send(id, extended, data, user)
}

pub(crate) fn can_get_state() -> Result<(i32, u8, u8), ZephyrResult> {
    hooks().can_get_state()
}

pub(crate) fn can_bitrate() -> u32 {
    hooks().can_bitrate()
}

pub(crate) fn pwm_set_led0(period_ns: u32, pulse_ns: u32) -> ZephyrResult {
    hooks().pwm_set_led0(period_ns, pulse_ns)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;
    use std::vec::Vec;

    struct Recorder {
        bytes: Mutex<Vec<u8>>,
    }

    impl Hooks for Recorder {
        fn uart_poll_out(&self, byte: u8) {
            self.bytes.lock().unwrap().push(byte);
        }

        fn cycles_per_sec(&self) -> u32 {
            170_000_000
        }

        fn cycle_get_64(&self) -> u64 {
            170_000_000 * 3 + 170_000
        }
    }

    /// The hooks are process-wide, so the tests run one at a time.
    static SERIAL: Mutex<()> = Mutex::new(());

    static RECORDER: Recorder = Recorder { bytes: Mutex::new(Vec::new()) };

    struct CanRecorder {
        sent: Mutex<Vec<usize>>,
    }

    impl Hooks for CanRecorder {
        fn can_send(
            &self,
            _id: u32,
            _extended: bool,
            data: &[u8],
            _user: *mut c_void,
        ) -> ZephyrResult {
            self.sent.lock().unwrap().push(data.len());
            0
        }
    }

    static CAN_RECORDER: CanRecorder = CanRecorder { sent: Mutex::new(Vec::new()) };

    // A classic frame carries at most 8 bytes: longer data never reaches Zephyr.
    #[test]
    fn can_send_refuses_more_than_eight_bytes() {
        let _serial = SERIAL.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        install(&CAN_RECORDER);
        assert_eq!(crate::can_send(0x558, false, &[0; 8], core::ptr::null_mut()), 0);
        assert_eq!(crate::can_send(0x558, false, &[0; 9], core::ptr::null_mut()), -crate::EINVAL);
        assert_eq!(CAN_RECORDER.sent.lock().unwrap().as_slice(), [8]);
        reset();
    }

    #[test]
    fn calls_reach_the_installed_hooks() {
        let _serial = SERIAL.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        install(&RECORDER);
        crate::uart_poll_out(b'x');
        assert_eq!(RECORDER.bytes.lock().unwrap().as_slice(), b"x");
        assert_eq!(crate::cyc_to_us_floor32(crate::cycle_get_64()), 3_001_000);
        assert_eq!(crate::cyc_to_ms_floor32(crate::cycle_get_64()), 3_001);
        assert_eq!(crate::cyc_to_ns_floor64(170), 1_000);
        // Past 2^32 microseconds (about 71.6 minutes) only the 64-bit conversion holds.
        let two_hours = 170_000_000 * 7_200;
        assert_eq!(crate::cyc_to_us_floor64(two_hours), 7_200_000_000);
        reset();
        assert_eq!(crate::uart_poll_in(), UART_NO_BYTE);
        assert_eq!(crate::task_name(0), b"<undefined>");
    }
}
