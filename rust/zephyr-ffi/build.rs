// Copyright 2026 Mist Tecnologia LTDA. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

//! Sets `zephyr_basepri` on the Arm M-profile targets whose Zephyr `irq_lock` masks
//! interrupts through BASEPRI (ARMv7-M and ARMv8-M Mainline), so the lock can be inlined
//! there as Zephyr's own `arch_irq_lock` is.

fn main() {
    println!("cargo::rustc-check-cfg=cfg(zephyr_basepri)");
    println!("cargo::rerun-if-env-changed=TARGET");
    let target = std::env::var("TARGET").unwrap_or_default();
    if target.starts_with("thumbv7m-")
        || target.starts_with("thumbv7em-")
        || target.starts_with("thumbv8m.main-")
    {
        println!("cargo::rustc-cfg=zephyr_basepri");
    }
}
