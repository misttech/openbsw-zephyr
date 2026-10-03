# Unsafe code

The count of `unsafe` blocks, functions and impls per crate, outside tests, and why each
exists.

| Crate | `unsafe` | Why |
|---|---|---|
| `zephyr-ffi` | 36 | `target.rs`: one `unsafe extern "C"` block declaring the shim's 35 functions, and one block per wrapper calling into it (plain scalar calls, or pointers valid for the call). The host implementation has none. |
| `rustapp` | 1 | The panic handler's call to `rust_panic_wrap`. |
