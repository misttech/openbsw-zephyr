# OpenBSW's Zephyr demo in Rust

The Rust port of this repository's Zephyr adaptation libraries (`libs/asyncZephyr`,
`libs/bspZephyr`) and of `samples/demo_app`, on the `main-mss` branch. It builds on the
Rust OpenBSW libraries in the `rust/` tree of
[openbsw](https://github.com/misttech/openbsw) (branch `main-mss`), which it reaches by
relative path: both trees are checked out side by side, as `west` and Forkpoint's
`prebuilt/third_party/` lay them out.

| Directory | Contents |
|---|---|
| `zephyr-ffi/` | Rust's view of Zephyr: hand-written `extern "C"` declarations of the shim's functions, wrapped safely, with host stubs for tests |
| `async-zephyr/` | Port of `libs/asyncZephyr`: `TaskContext` (one async context on one thread, with its event object and one-shot timer), `ZephyrAdapter` (the platform binding), the `irq_lock` critical section, and the tracing hooks |
| `bsp-zephyr/` | Port of `libs/bspZephyr`: the system timer, the console `Stdio` and `ZephyrCanTransceiver` over Zephyr's CAN API |
| `cmake/` | `openbsw_rust_application()`, which builds the Cargo package of a Zephyr application and links it, and the C `main` that calls `rust_main()` |
| `samples/demo_app/` | The demo application: a Zephyr app whose only C file is `src/zephyr_shim.c` |

## No bindings

The Rust code does not depend on the `zephyr` crate, on `zephyr-sys`, or on bindgen.
`samples/demo_app/src/zephyr_shim.c` owns every kernel object the demo needs (the five
task threads and their stacks, events, timers, the console and CAN devices, the PWM LED)
and exports `cpp_`-prefixed functions that take and return plain scalars. Rust exports
`rust_`-prefixed callbacks (timer expiry, CAN receive and transmit completion, the tracing
hooks). The shim is argument marshalling plus one Zephyr call per function; the state
machines are Rust. So the build needs no libclang, and does not depend on the layout of
`struct k_thread` or `struct can_frame`.

## Building

The application builds like any Zephyr application, with this repository's `cmake/` as
the only addition to its `CMakeLists.txt`:

```sh
west build -b nucleo_g474re openbsw-zephyr/rust/samples/demo_app
```

Forkpoint drives the same CMake build without `west` and runs the result in its
deterministic virtual MCU next to the C++ demo (`examples/nucleo-g474re-openbsw-rust-demo`
in the Forkpoint repository). The Rust target is the board's: `thumbv7em-none-eabi` for
the NUCLEO-G474RE (Zephyr builds it without `CONFIG_FPU`); `rustup target add` installs
it. The host tests run with:

```sh
cd rust
cargo test --workspace --exclude rustapp
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --check
cargo build -p rustapp --target thumbv7em-none-eabi --release
```

See `PORTING.md` for how the C++ is ported and `UNSAFE.md` for the `unsafe` count per
crate.
