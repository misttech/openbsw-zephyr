// Copyright 2026 Mist Tecnologia LTDA. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

//! Rust's view of Zephyr, through a hand-written C shim.
//!
//! The Rust application does not depend on the `zephyr` crate or on bindgen. Instead
//! `samples/demo_app/src/zephyr_shim.c` owns every kernel object the demo needs (thread
//! stacks and control blocks, events, timers, the console and CAN devices, the PWM LED)
//! and exports `cpp_`-prefixed functions that take and return plain scalars. This crate
//! declares them and wraps each in a safe function.
//!
//! On the host (tests), the same functions call a [`Hooks`] implementation installed by
//! the test, so the glue crates run unchanged. The default hooks do nothing and return
//! neutral values.
//!
//! Callbacks the shim makes into Rust (`rust_task_timer_expired`, `rust_can_rx`,
//! `rust_can_tx_done`, the tracing hooks) are declared by the crates that define them.

#![no_std]
#![deny(unsafe_op_in_unsafe_fn)]
#![warn(missing_docs)]

use core::ffi::c_void;

#[cfg(not(target_os = "none"))]
pub mod host;
#[cfg(target_os = "none")]
mod target;

#[cfg(not(target_os = "none"))]
use host as imp;
#[cfg(target_os = "none")]
use target as imp;

/// The entry function of a task created with [`task_create`].
pub type TaskEntry = extern "C" fn(arg: *mut c_void);

/// The result of a Zephyr call: zero on success, a negative errno otherwise.
pub type ZephyrResult = i32;

/// The value [`uart_poll_in`] returns when no byte is waiting: the BSP stores `-1` in an
/// `unsigned char`.
pub const UART_NO_BYTE: i32 = 255;

/// The most data bytes a classic CAN frame carries (Zephyr's `CAN_MAX_DLEN`).
pub const CAN_MAX_DLEN: usize = 8;

/// Invalid argument.
pub const EINVAL: i32 = 22;

/// Zephyr's `EAGAIN`: the controller's queue is full.
pub const EAGAIN: i32 = 11;
/// Zephyr's `ENOSPC`: no filter slot is left.
pub const ENOSPC: i32 = 28;
/// Zephyr's `ENOTSUP`: the filter is not supported.
pub const ENOTSUP: i32 = 134;
/// Zephyr's `CAN_STATE_BUS_OFF` in `enum can_state`, as [`can_get_state`] reports it.
pub const CAN_STATE_BUS_OFF: i32 = 3;

/// Create the thread of async context `context` (not started): `name`, Zephyr
/// `priority`, running `entry(arg)`. The shim owns the stack and control block.
pub fn task_create(
    context: u32,
    name: &'static core::ffi::CStr,
    priority: i32,
    entry: TaskEntry,
    arg: *mut c_void,
) {
    imp::task_create(context, name, priority, entry, arg);
}

/// Start the thread of `context`.
pub fn task_start(context: u32) {
    imp::task_start(context);
}

/// The name of the thread of `context`, as given to [`task_create`].
pub fn task_name(context: u32) -> &'static [u8] {
    imp::task_name(context)
}

/// The stack size of the thread of `context`.
pub fn task_stack_size(context: u32) -> u32 {
    imp::task_stack_size(context)
}

/// The unused stack of the thread of `context`, or `None` when Zephyr cannot tell.
pub fn task_stack_unused(context: u32) -> Option<u32> {
    imp::task_stack_unused(context)
}

/// Post `mask` to the event of `context`.
pub fn event_post(context: u32, mask: u32) {
    imp::event_post(context, mask);
}

/// Wait forever for any bit of `mask` on the event of `context`; returns the bits set,
/// without clearing them.
pub fn event_wait(context: u32, mask: u32) -> u32 {
    imp::event_wait(context, mask)
}

/// Clear `mask` on the event of `context`.
pub fn event_clear(context: u32, mask: u32) {
    imp::event_clear(context, mask);
}

/// Start the one-shot timer of `context` to expire in `microseconds`; the shim then calls
/// `rust_task_timer_expired(context)`.
pub fn timer_start_us(context: u32, microseconds: u32) {
    imp::timer_start_us(context, microseconds);
}

/// Stop the timer of `context`.
pub fn timer_stop(context: u32) {
    imp::timer_stop(context);
}

/// Lock interrupts; returns the key for [`irq_unlock`].
#[inline]
pub fn irq_lock() -> u32 {
    imp::irq_lock()
}

/// Unlock interrupts with the key from [`irq_lock`].
#[inline]
pub fn irq_unlock(key: u32) {
    imp::irq_unlock(key);
}

/// Whether the caller runs in an interrupt.
pub fn is_in_isr() -> bool {
    imp::is_in_isr()
}

/// The Zephyr priority of the current thread.
pub fn current_priority() -> i32 {
    imp::current_priority()
}

/// The Zephyr priority of the thread the scheduler is switching to or from, for the
/// tracing hooks.
pub fn sched_current_priority() -> i32 {
    imp::sched_current_priority()
}

/// Sleep for `milliseconds`.
pub fn msleep(milliseconds: i32) {
    imp::msleep(milliseconds);
}

/// Reboot the system (`SYS_REBOOT_COLD`).
pub fn reboot_cold() -> ! {
    imp::reboot_cold()
}

/// The low 32 bits of the free-running cycle counter, as `k_cycle_get_32` reads them.
pub fn cycle_get_32() -> u32 {
    imp::cycle_get_32()
}

/// The free-running cycle counter.
pub fn cycle_get_64() -> u64 {
    imp::cycle_get_64()
}

/// Cycles to microseconds, rounded down, as `k_cyc_to_us_floor32` does.
pub fn cyc_to_us_floor32(cycles: u64) -> u32 {
    imp::cyc_to_us_floor32(cycles)
}

/// Cycles to microseconds, rounded down, in 64 bits, as `k_cyc_to_us_floor64` does.
pub fn cyc_to_us_floor64(cycles: u64) -> u64 {
    imp::cyc_to_us_floor64(cycles)
}

/// Cycles to milliseconds, rounded down, as `k_cyc_to_ms_floor32` does.
pub fn cyc_to_ms_floor32(cycles: u64) -> u32 {
    imp::cyc_to_ms_floor32(cycles)
}

/// Cycles to nanoseconds, rounded down, as `k_cyc_to_ns_floor64` does.
pub fn cyc_to_ns_floor64(cycles: u64) -> u64 {
    imp::cyc_to_ns_floor64(cycles)
}

/// The cycle counter's frequency.
pub fn cycles_per_sec() -> u32 {
    imp::cycles_per_sec()
}

/// The next byte from the console, or [`UART_NO_BYTE`].
pub fn uart_poll_in() -> i32 {
    imp::uart_poll_in()
}

/// Write one byte to the console.
pub fn uart_poll_out(byte: u8) {
    imp::uart_poll_out(byte);
}

/// Whether the CAN device is ready.
pub fn can_device_is_ready() -> bool {
    imp::can_device_is_ready()
}

/// Put the CAN controller in normal mode.
pub fn can_set_mode_normal() -> ZephyrResult {
    imp::can_set_mode_normal()
}

/// Start the CAN controller.
pub fn can_start() -> ZephyrResult {
    imp::can_start()
}

/// Stop the CAN controller.
pub fn can_stop() -> ZephyrResult {
    imp::can_stop()
}

/// Add a receive filter matching every frame; received frames reach
/// `rust_can_rx(id, extended, length, data, user)`. Returns the filter id, or a negative
/// errno.
pub fn can_add_rx_filter_all(user: *mut c_void) -> i32 {
    imp::can_add_rx_filter_all(user)
}

/// Remove the receive filter `filter_id`.
pub fn can_remove_rx_filter(filter_id: i32) {
    imp::can_remove_rx_filter(filter_id);
}

/// Queue a frame without waiting; `rust_can_tx_done(error, user)` reports completion.
/// A classic frame carries at most [`CAN_MAX_DLEN`] bytes; longer `data` is refused
/// with `-EINVAL` and nothing is sent.
pub fn can_send(id: u32, extended: bool, data: &[u8], user: *mut c_void) -> ZephyrResult {
    if data.len() > CAN_MAX_DLEN {
        return -EINVAL;
    }
    imp::can_send(id, extended, data, user)
}

/// The controller state and error counters: `(state, tx_error_count, rx_error_count)`.
pub fn can_get_state() -> Result<(i32, u8, u8), ZephyrResult> {
    imp::can_get_state()
}

/// The CAN bus bit rate the Device Tree gives the chosen controller.
pub fn can_bitrate() -> u32 {
    imp::can_bitrate()
}

/// Set the LED's PWM period and pulse, in nanoseconds.
pub fn pwm_set_led0(period_ns: u32, pulse_ns: u32) -> ZephyrResult {
    imp::pwm_set_led0(period_ns, pulse_ns)
}
