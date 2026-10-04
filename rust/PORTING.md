# Porting the Zephyr adaptation

The rules of `openbsw/rust/PORTING.md` apply: a direct translation of each C++
class with the same behavior, test parity with the upstream tests, no new
failure modes, `// SAFETY:` on every `unsafe`. On top of them:

1. **Zephyr through the shim only.** Rust calls Zephyr through `zephyr_ffi`,
   whose functions the shim implements. A new Zephyr call is one `cpp_` function
   in `zephyr_shim.c` (declarative: marshal the arguments, make the call), one
   `extern "C"` declaration and one safe wrapper in `zephyr-ffi/src/target.rs`,
   one method with a neutral default in `zephyr-ffi/src/host.rs`'s `Hooks`, and
   one entry in `lib.rs`.
2. **Callbacks are `rust_` functions.** Zephyr calls into Rust only through
   `#[unsafe(no_mangle)] extern "C"` functions named `rust_*`, which the shim
   declares and gives weak fallbacks so that a partial application still links.
3. **Kernel objects live in C.** Thread stacks, control blocks, events and
   timers are statics in the shim, indexed by async context; Rust never sees
   their layout.
4. **Scheduling order is the contract.** `TaskContext` and `ZephyrAdapter` are
   mirrored one to one: the same event bits, the same order of handling
   runnables and timeouts, the same priorities, so the console output matches
   the C++ build line for line.
5. **Host tests through hooks.** The glue crates are tested on the host by
   installing a `zephyr_ffi::host::Hooks` implementation that records the calls
   and answers them.
6. **Stacks and optimization as the C++ build.** The Rust profiles optimize for
   speed (`opt-level = 3` with fat LTO), and the shim compiles at `-O2`: Zephyr
   builds itself with `-Os`, but OpenBSW's CMake builds its C++ libraries at
   `-O2`, and those headers are inlined across libraries, which Rust does across
   crates only with LTO. The hot paths are shaped as the C++ ones are:
   `irq_lock` is inlined on BASEPRI cores, the run time monitor reads its clock
   through a type parameter, and the timer list keeps each timeout's link beside
   it, so walking the list makes no calls. Measured in Forkpoint over 10 s of
   the demo, the Rust image runs within 1% of the C++ instruction count. The
   shim gives each task the stack `main.cpp` gives it, so `stats stack` reads
   the same, with two exceptions measured in Forkpoint: the CAN task has 2 KB
   instead of 1 KB, because the Rust DoCAN receive path (frame decoding, the
   receiver's state machine, the router's buffer and its log line) uses about
   1.4 KB where the C++ uses 0.8 KB, and the background task has 2 KB instead of
   1 KB, because the `stats` command it runs uses about 1.1 KB. Measure before
   changing a stack: run the firmware in Forkpoint with a debugger attached and
   read the task's stack for the end of its `0xAA` fill (`CONFIG_INIT_STACKS`),
   or type `stats stack` on the console.
