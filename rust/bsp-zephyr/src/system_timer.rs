// Copyright 2026 Mist Tecnologia LTDA. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

//! The clocks, ported from `bsp/SystemTimer.cpp`: Zephyr's cycle counter in the units
//! OpenBSW asks for.

/// Microseconds since boot, truncated to 32 bits (`getSystemTimeUs32Bit`).
pub fn system_time_us32() -> u32 {
    zephyr_ffi::cyc_to_us_floor32(zephyr_ffi::cycle_get_64())
}

/// Milliseconds since boot, truncated to 32 bits (`getSystemTimeMs32Bit`).
pub fn system_time_ms32() -> u32 {
    zephyr_ffi::cyc_to_ms_floor32(zephyr_ffi::cycle_get_64())
}

/// Nanoseconds since boot (`getSystemTimeNs`), also the logger's clock.
pub fn system_time_ns() -> u64 {
    zephyr_ffi::cyc_to_ns_floor64(zephyr_ffi::cycle_get_64())
}

/// The cycle counter, truncated to 32 bits (`getSystemTicks32Bit`).
pub fn system_ticks32() -> u32 {
    zephyr_ffi::cycle_get_64() as u32
}

/// Cycles to nanoseconds (`systemTicksToTimeNs`).
pub fn ticks_to_ns(ticks: u64) -> u64 {
    zephyr_ffi::cyc_to_ns_floor64(ticks)
}

/// Cycles to microseconds in 64 bits (`systemTicksToTimeUs`, `k_cyc_to_us_floor64`), so
/// a span past 2^32 microseconds does not wrap.
pub fn ticks_to_us(ticks: u64) -> u64 {
    zephyr_ffi::cyc_to_us_floor64(ticks)
}
