# Copyright 2026 Mist Tecnologia LTDA. All rights reserved.
# SPDX-License-Identifier: Apache-2.0

# Build the Cargo package in the application directory as a static library named
# `rustapp` and link it into the Zephyr application, with a C main that calls
# `rust_main()`. The Rust code reaches Zephyr through the application's C shim, so no
# bindings are generated and no libclang is needed; the only Rust toolchain
# requirement is the bare-metal target for the board's CPU.
#
# Usage, after find_package(Zephyr) and project():
#
#   include(${CMAKE_CURRENT_SOURCE_DIR}/../../cmake/rust_app.cmake)
#   target_sources(app PRIVATE src/zephyr_shim.c)
#   openbsw_rust_application()
#
# RUST_CARGO_ARGS adds arguments to the cargo invocation (for instance --locked).
function(openbsw_rust_application)
  if(CONFIG_CPU_CORTEX_M4 OR CONFIG_CPU_CORTEX_M7)
    if(CONFIG_FP_HARDABI)
      set(rust_target "thumbv7em-none-eabihf")
    else()
      set(rust_target "thumbv7em-none-eabi")
    endif()
  elseif(CONFIG_CPU_CORTEX_M33)
    if(CONFIG_FP_HARDABI)
      set(rust_target "thumbv8m.main-none-eabihf")
    else()
      set(rust_target "thumbv8m.main-none-eabi")
    endif()
  elseif(CONFIG_CPU_CORTEX_M3)
    set(rust_target "thumbv7m-none-eabi")
  elseif(CONFIG_CPU_CORTEX_M0 OR CONFIG_CPU_CORTEX_M0PLUS)
    set(rust_target "thumbv6m-none-eabi")
  else()
    message(FATAL_ERROR "openbsw_rust_application: no Rust target for this CPU")
  endif()
  if(CONFIG_DEBUG)
    set(profile "debug")
    set(profile_arg "")
  else()
    set(profile "release")
    set(profile_arg "--release")
  endif()
  set(target_dir "${CMAKE_CURRENT_BINARY_DIR}/rust/target")
  set(rust_library "${target_dir}/${rust_target}/${profile}/librustapp.a")
  # An output that is never created makes CMake run cargo on every build; cargo itself
  # decides what is stale.
  set(always_run "${CMAKE_CURRENT_BINARY_DIR}/rust/always-run-cargo")
  add_custom_command(
    OUTPUT ${always_run}
    BYPRODUCTS ${rust_library}
    USES_TERMINAL
    COMMAND cargo build ${profile_arg} --target ${rust_target} --target-dir ${target_dir} ${RUST_CARGO_ARGS}
    WORKING_DIRECTORY ${CMAKE_CURRENT_SOURCE_DIR}
    COMMENT "Building the Rust application"
  )
  add_custom_target(rustapp ALL DEPENDS ${always_run} offsets_h syscall_list_h_target
    driver_validation_h_target kobj_types_h_target)
  # The runtime library goes first: librustapp.a carries its own compiler builtins, and
  # the linker takes them from whichever archive comes first.
  target_link_libraries(app PUBLIC $<TARGET_PROPERTY:linker,rt_library> ${rust_library})
  add_dependencies(app rustapp)
  target_sources(app PRIVATE ${CMAKE_CURRENT_FUNCTION_LIST_DIR}/rust_main.c)
endfunction()
