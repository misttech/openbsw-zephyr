// Copyright 2026 Mist Tecnologia LTDA. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

//! The critical section, ported from `async/Lock.h` and `async/ModifiableLock.h`:
//! Zephyr's `irq_lock`/`irq_unlock`.

use openbsw_async::{ModifiableLock, RawLock, ScopedLock};

/// `irq_lock`/`irq_unlock` through the shim.
pub struct IrqRawLock;

impl RawLock for IrqRawLock {
    fn acquire() -> u32 {
        zephyr_ffi::irq_lock()
    }

    fn release(key: u32) {
        zephyr_ffi::irq_unlock(key);
    }
}

/// `async::LockType`: a critical section held for a scope.
pub type LockType = ScopedLock<IrqRawLock>;

/// `async::ModifiableLockType`: a critical section that can be released and retaken.
pub type ModifiableLockType = ModifiableLock<IrqRawLock>;
