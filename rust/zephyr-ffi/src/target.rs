// Copyright 2026 Mist Tecnologia LTDA. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

//! The bare-metal implementation: calls into `zephyr_shim.c`.

use core::ffi::{CStr, c_char, c_void};

use crate::{TaskEntry, ZephyrResult};

unsafe extern "C" {
    fn cpp_task_create(
        context: u32,
        name: *const c_char,
        priority: i32,
        entry: TaskEntry,
        arg: *mut c_void,
    );
    fn cpp_task_start(context: u32);
    fn cpp_task_name(context: u32) -> *const c_char;
    fn cpp_task_stack_size(context: u32) -> u32;
    fn cpp_task_stack_unused(context: u32, unused: *mut u32) -> i32;
    fn cpp_event_post(context: u32, mask: u32);
    fn cpp_event_wait(context: u32, mask: u32) -> u32;
    fn cpp_event_clear(context: u32, mask: u32);
    fn cpp_timer_start_us(context: u32, microseconds: u32);
    fn cpp_timer_stop(context: u32);
    fn cpp_irq_lock() -> u32;
    fn cpp_irq_unlock(key: u32);
    fn cpp_is_in_isr() -> bool;
    fn cpp_current_priority() -> i32;
    fn cpp_sched_current_priority() -> i32;
    fn cpp_msleep(milliseconds: i32) -> i32;
    fn cpp_reboot_cold() -> !;
    fn cpp_cycle_get_64() -> u64;
    fn cpp_cyc_to_us_floor32(cycles: u64) -> u32;
    fn cpp_cyc_to_us_floor64(cycles: u64) -> u64;
    fn cpp_cyc_to_ms_floor32(cycles: u64) -> u32;
    fn cpp_cyc_to_ns_floor64(cycles: u64) -> u64;
    fn cpp_cycles_per_sec() -> u32;
    fn cpp_uart_poll_in() -> i32;
    fn cpp_uart_poll_out(byte: u8);
    fn cpp_can_device_is_ready() -> bool;
    fn cpp_can_set_mode_normal() -> i32;
    fn cpp_can_start() -> i32;
    fn cpp_can_stop() -> i32;
    fn cpp_can_add_rx_filter_all() -> i32;
    fn cpp_can_remove_rx_filter(filter_id: i32);
    fn cpp_can_send(id: u32, extended: bool, dlc: u8, data: *const u8, user: *mut c_void) -> i32;
    fn cpp_can_get_state(state: *mut i32, tx_error_count: *mut u8, rx_error_count: *mut u8) -> i32;
    fn cpp_pwm_set_led0(period_ns: u32, pulse_ns: u32) -> i32;
}

// Every function below is a plain call into the shim: the arguments are scalars or
// pointers that stay valid for the call, and the shim has no preconditions beyond the
// ones each wrapper's documentation states.

pub(crate) fn task_create(
    context: u32,
    name: &'static CStr,
    priority: i32,
    entry: TaskEntry,
    arg: *mut c_void,
) {
    // SAFETY: `name` is NUL-terminated and lives for the program; the shim stores the
    // pointer with the thread.
    unsafe { cpp_task_create(context, name.as_ptr(), priority, entry, arg) }
}

pub(crate) fn task_start(context: u32) {
    // SAFETY: plain scalar call.
    unsafe { cpp_task_start(context) }
}

pub(crate) fn task_name(context: u32) -> &'static [u8] {
    // SAFETY: the shim returns the NUL-terminated name given to `task_create`, which
    // lives for the program, or a static placeholder.
    unsafe { CStr::from_ptr(cpp_task_name(context)) }.to_bytes()
}

pub(crate) fn task_stack_size(context: u32) -> u32 {
    // SAFETY: plain scalar call.
    unsafe { cpp_task_stack_size(context) }
}

pub(crate) fn task_stack_unused(context: u32) -> Option<u32> {
    let mut unused: u32 = 0;
    // SAFETY: `unused` outlives the call.
    let result = unsafe { cpp_task_stack_unused(context, &mut unused) };
    (result == 0).then_some(unused)
}

pub(crate) fn event_post(context: u32, mask: u32) {
    // SAFETY: plain scalar call.
    unsafe { cpp_event_post(context, mask) }
}

pub(crate) fn event_wait(context: u32, mask: u32) -> u32 {
    // SAFETY: plain scalar call.
    unsafe { cpp_event_wait(context, mask) }
}

pub(crate) fn event_clear(context: u32, mask: u32) {
    // SAFETY: plain scalar call.
    unsafe { cpp_event_clear(context, mask) }
}

pub(crate) fn timer_start_us(context: u32, microseconds: u32) {
    // SAFETY: plain scalar call.
    unsafe { cpp_timer_start_us(context, microseconds) }
}

pub(crate) fn timer_stop(context: u32) {
    // SAFETY: plain scalar call.
    unsafe { cpp_timer_stop(context) }
}

pub(crate) fn irq_lock() -> u32 {
    // SAFETY: plain scalar call.
    unsafe { cpp_irq_lock() }
}

pub(crate) fn irq_unlock(key: u32) {
    // SAFETY: plain scalar call.
    unsafe { cpp_irq_unlock(key) }
}

pub(crate) fn is_in_isr() -> bool {
    // SAFETY: plain scalar call.
    unsafe { cpp_is_in_isr() }
}

pub(crate) fn current_priority() -> i32 {
    // SAFETY: plain scalar call.
    unsafe { cpp_current_priority() }
}

pub(crate) fn sched_current_priority() -> i32 {
    // SAFETY: plain scalar call.
    unsafe { cpp_sched_current_priority() }
}

pub(crate) fn msleep(milliseconds: i32) {
    // SAFETY: plain scalar call.
    unsafe { cpp_msleep(milliseconds) };
}

pub(crate) fn reboot_cold() -> ! {
    // SAFETY: plain call that does not return.
    unsafe { cpp_reboot_cold() }
}

pub(crate) fn cycle_get_64() -> u64 {
    // SAFETY: plain scalar call.
    unsafe { cpp_cycle_get_64() }
}

pub(crate) fn cyc_to_us_floor32(cycles: u64) -> u32 {
    // SAFETY: plain scalar call.
    unsafe { cpp_cyc_to_us_floor32(cycles) }
}

pub(crate) fn cyc_to_us_floor64(cycles: u64) -> u64 {
    // SAFETY: plain scalar call.
    unsafe { cpp_cyc_to_us_floor64(cycles) }
}

pub(crate) fn cyc_to_ms_floor32(cycles: u64) -> u32 {
    // SAFETY: plain scalar call.
    unsafe { cpp_cyc_to_ms_floor32(cycles) }
}

pub(crate) fn cyc_to_ns_floor64(cycles: u64) -> u64 {
    // SAFETY: plain scalar call.
    unsafe { cpp_cyc_to_ns_floor64(cycles) }
}

pub(crate) fn cycles_per_sec() -> u32 {
    // SAFETY: plain scalar call.
    unsafe { cpp_cycles_per_sec() }
}

pub(crate) fn uart_poll_in() -> i32 {
    // SAFETY: plain scalar call.
    unsafe { cpp_uart_poll_in() }
}

pub(crate) fn uart_poll_out(byte: u8) {
    // SAFETY: plain scalar call.
    unsafe { cpp_uart_poll_out(byte) }
}

pub(crate) fn can_device_is_ready() -> bool {
    // SAFETY: plain scalar call.
    unsafe { cpp_can_device_is_ready() }
}

pub(crate) fn can_set_mode_normal() -> ZephyrResult {
    // SAFETY: plain scalar call.
    unsafe { cpp_can_set_mode_normal() }
}

pub(crate) fn can_start() -> ZephyrResult {
    // SAFETY: plain scalar call.
    unsafe { cpp_can_start() }
}

pub(crate) fn can_stop() -> ZephyrResult {
    // SAFETY: plain scalar call.
    unsafe { cpp_can_stop() }
}

pub(crate) fn can_add_rx_filter_all() -> i32 {
    // SAFETY: plain scalar call.
    unsafe { cpp_can_add_rx_filter_all() }
}

pub(crate) fn can_remove_rx_filter(filter_id: i32) {
    // SAFETY: plain scalar call.
    unsafe { cpp_can_remove_rx_filter(filter_id) }
}

pub(crate) fn can_send(id: u32, extended: bool, data: &[u8], user: *mut c_void) -> ZephyrResult {
    // The public wrapper refuses more than `CAN_MAX_DLEN` bytes, so this fits a `u8`.
    let dlc = data.len() as u8;
    // SAFETY: `data` is valid for `dlc` bytes for the call, at most `CAN_MAX_DLEN`, and
    // the shim refuses more; it copies the bytes into its own frame before returning.
    unsafe { cpp_can_send(id, extended, dlc, data.as_ptr(), user) }
}

pub(crate) fn can_get_state() -> Result<(i32, u8, u8), ZephyrResult> {
    let mut state: i32 = 0;
    let mut tx_error_count: u8 = 0;
    let mut rx_error_count: u8 = 0;
    // SAFETY: the three out-pointers outlive the call.
    let result = unsafe { cpp_can_get_state(&mut state, &mut tx_error_count, &mut rx_error_count) };
    if result == 0 { Ok((state, tx_error_count, rx_error_count)) } else { Err(result) }
}

pub(crate) fn pwm_set_led0(period_ns: u32, pulse_ns: u32) -> ZephyrResult {
    // SAFETY: plain scalar call.
    unsafe { cpp_pwm_set_led0(period_ns, pulse_ns) }
}
