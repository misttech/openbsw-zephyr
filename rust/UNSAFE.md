# Unsafe code

The count of `unsafe` blocks, functions and impls per crate, outside tests, and why each
exists.

| Crate | `unsafe` | Why |
|---|---|---|
| `zephyr-ffi` | 36 | `target.rs`: one `unsafe extern "C"` block declaring the shim's 35 functions, and one block per wrapper calling into it (plain scalar calls, or pointers valid for the call). The host implementation has none. |
| `async-zephyr` | 11 | `context.rs`: `unsafe impl Sync for TaskContext` (its cells are written before the threads start) and the cast of the thread argument back to the `'static` context that `create_task` passed. `adapter.rs` and `hook.rs`: two `RacyCell`s (the context table the timer callback reaches, the installed context hook) read and written during startup, and the five `#[unsafe(no_mangle)]` callbacks the shim calls (`rust_task_timer_expired`, `rust_async_{enter,leave}_task`, `rust_async_{enter,leave}_isr_group`). |
| `bsp-zephyr` | 0 | |
| `rustapp` | 4 | The `#[unsafe(no_mangle)]` entry point `rust_main`; the panic handler's declaration of and call to `rust_panic_wrap`; `unsafe impl Sync for DemoSystem` (its counter is only touched on the demo context). |
