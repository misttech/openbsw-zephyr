// Copyright 2026 Mist Tecnologia LTDA. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

// The C entry points of a Rust Zephyr application built with rust_app.cmake.

#include <zephyr/kernel.h>

extern void rust_main(void);

int main(void)
{
	rust_main();
	return 0;
}

// k_panic() is a macro, so the Rust panic handler calls this instead.
void rust_panic_wrap(void)
{
	k_panic();
}
