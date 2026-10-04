# Copyright 2026 Mist Tecnologia LTDA. All rights reserved.
# SPDX-License-Identifier: Apache-2.0

cmake_minimum_required(VERSION 3.20.0)

find_package(Zephyr REQUIRED HINTS $ENV{ZEPHYR_BASE})

project(demo_app_rust)

include(${CMAKE_CURRENT_SOURCE_DIR}/../../cmake/rust_app.cmake)

# The only C file: it owns the kernel objects and forwards plain-scalar calls to Zephyr.
target_sources(app PRIVATE src/zephyr_shim.c)
# Zephyr builds for size, but OpenBSW's own CMake builds its libraries at -O2; the shim
# sits on the same hot paths (timer starts, time conversions), so it gets -O2 as well.
set_source_files_properties(src/zephyr_shim.c PROPERTIES COMPILE_OPTIONS -O2)

openbsw_rust_application()
