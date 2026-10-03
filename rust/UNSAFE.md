# Unsafe code

The count of `unsafe` blocks, functions and impls per crate, outside tests, and why each
exists.

| Crate | `unsafe` | Why |
|---|---|---|
| `zephyr-ffi` | 37 | `target.rs`: one `unsafe extern "C"` block declaring the shim's 36 functions, and one block per wrapper calling into it (plain scalar calls, or pointers valid for the call). The host implementation has none. |
| `async-zephyr` | 11 | `context.rs`: `unsafe impl Sync for TaskContext` (its cells are written before the threads start) and the cast of the thread argument back to the `'static` context that `create_task` passed. `adapter.rs` and `hook.rs`: two `RacyCell`s (the context table the timer callback reaches, the installed context hook) read and written during startup, and the five `#[unsafe(no_mangle)]` callbacks the shim calls (`rust_task_timer_expired`, `rust_async_{enter,leave}_task`, `rust_async_{enter,leave}_isr_group`). |
| `bsp-zephyr` | 5 | `can.rs`: `unsafe impl Sync for ZephyrCanTransceiver` (its queues and counters are changed under the platform lock, its state on its context); the two `#[unsafe(no_mangle)]` callbacks the shim calls (`rust_can_rx`, `rust_can_tx_done`) and, in each, the block that turns the `user` pointer the transceiver registered back into the `'static` transceiver (and the frame data into a slice). |
| `rustapp` | 4 | The `#[unsafe(no_mangle)]` entry point `rust_main`; the panic handler's declaration of and call to `rust_panic_wrap`; `unsafe impl Sync for DemoSystem` (its counter is only touched on the demo context). |
