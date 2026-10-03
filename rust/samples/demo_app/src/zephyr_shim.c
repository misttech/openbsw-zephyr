// Copyright 2026 Mist Tecnologia LTDA. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

// The C shim between the Rust application and Zephyr.
//
// Rust does not depend on Zephyr's headers or on generated bindings: this file owns every
// kernel object the demo needs and exports `cpp_`-prefixed functions that take and
// return plain scalars. Each function is argument marshalling plus one Zephyr call; the
// state machines live in Rust. Rust exports the `rust_`-prefixed callbacks; weak
// fallbacks here let an application that does not define them link.

#include <string.h>

#include <zephyr/device.h>
#include <zephyr/drivers/uart.h>
#include <zephyr/init.h>
#include <zephyr/kernel.h>
#include <zephyr/sys/reboot.h>
#include <cmsis_core.h>
#include <tracing_user.h>

#ifdef CONFIG_CAN
#include <zephyr/drivers/can.h>
#endif
#ifdef CONFIG_PWM
#include <zephyr/drivers/pwm.h>
#endif

// The async contexts of async/Config.h, in priority order, and their stacks as
// demo_app/src/main.cpp defines them.
#define TASK_COUNT 5

K_THREAD_STACK_DEFINE(sysadmin_stack, 1024);
K_THREAD_STACK_DEFINE(can_stack, 1024);
K_THREAD_STACK_DEFINE(demo_stack, 4 * 1024);
K_THREAD_STACK_DEFINE(uds_stack, 2 * 1024);
K_THREAD_STACK_DEFINE(background_stack, 1024);

static k_thread_stack_t *const stacks[TASK_COUNT] = {
	sysadmin_stack, can_stack, demo_stack, uds_stack, background_stack,
};
static const size_t stack_sizes[TASK_COUNT] = {
	K_THREAD_STACK_SIZEOF(sysadmin_stack), K_THREAD_STACK_SIZEOF(can_stack),
	K_THREAD_STACK_SIZEOF(demo_stack),     K_THREAD_STACK_SIZEOF(uds_stack),
	K_THREAD_STACK_SIZEOF(background_stack),
};
static struct k_thread threads[TASK_COUNT];
static k_tid_t thread_ids[TASK_COUNT];
static struct k_event events[TASK_COUNT];
static struct k_timer timers[TASK_COUNT];
static bool application_initialized;

typedef void (*rust_task_entry_t)(void *arg);

// Callbacks into Rust.
extern void rust_task_timer_expired(uint32_t context);
extern void rust_async_enter_task(int32_t context);
extern void rust_async_leave_task(int32_t context);
extern void rust_async_enter_isr_group(uint32_t group);
extern void rust_async_leave_isr_group(uint32_t group);
#ifdef CONFIG_CAN
extern void rust_can_rx(uint32_t id, bool extended, uint8_t length, const uint8_t *data, void *user);
extern void rust_can_tx_done(int error, void *user);
#endif

__weak void rust_task_timer_expired(uint32_t context) { ARG_UNUSED(context); }
__weak void rust_async_enter_task(int32_t context) { ARG_UNUSED(context); }
__weak void rust_async_leave_task(int32_t context) { ARG_UNUSED(context); }
__weak void rust_async_enter_isr_group(uint32_t group) { ARG_UNUSED(group); }
__weak void rust_async_leave_isr_group(uint32_t group) { ARG_UNUSED(group); }
#ifdef CONFIG_CAN
__weak void rust_can_rx(uint32_t id, bool extended, uint8_t length, const uint8_t *data, void *user)
{
	ARG_UNUSED(id);
	ARG_UNUSED(extended);
	ARG_UNUSED(length);
	ARG_UNUSED(data);
	ARG_UNUSED(user);
}
__weak void rust_can_tx_done(int error, void *user)
{
	ARG_UNUSED(error);
	ARG_UNUSED(user);
}
#endif

static int init_objects(void)
{
	for (uint32_t i = 0; i < TASK_COUNT; ++i) {
		k_event_init(&events[i]);
	}
	// Constructors have run once the APPLICATION init level is reached: only then may
	// the tracing hooks call into the async framework (as TraceHooks.cpp does).
	application_initialized = true;
	return 0;
}
SYS_INIT(init_objects, APPLICATION, CONFIG_KERNEL_INIT_PRIORITY_DEFAULT);

// Threads and events

static void task_entry(void *p1, void *p2, void *p3)
{
	ARG_UNUSED(p3);
	((rust_task_entry_t)p1)(p2);
}

void cpp_task_create(uint32_t context, const char *name, int32_t priority, rust_task_entry_t entry,
		     void *arg)
{
	thread_ids[context] = k_thread_create(&threads[context], stacks[context], stack_sizes[context],
					      task_entry, (void *)entry, arg, NULL, priority, 0,
					      K_FOREVER);
	k_thread_name_set(thread_ids[context], name);
}

void cpp_task_start(uint32_t context)
{
	if (thread_ids[context] != NULL) {
		k_thread_start(thread_ids[context]);
	}
}

const char *cpp_task_name(uint32_t context)
{
	if (thread_ids[context] != NULL) {
		return k_thread_name_get(thread_ids[context]);
	}
	return "<undefined>";
}

uint32_t cpp_task_stack_size(uint32_t context)
{
	return thread_ids[context] != NULL ? thread_ids[context]->stack_info.size : 0;
}

int32_t cpp_task_stack_unused(uint32_t context, uint32_t *unused)
{
	size_t value = 0;
	int result = k_thread_stack_space_get(thread_ids[context], &value);
	*unused = (uint32_t)value;
	return result;
}

void cpp_event_post(uint32_t context, uint32_t mask) { k_event_post(&events[context], mask); }

uint32_t cpp_event_wait(uint32_t context, uint32_t mask)
{
	return k_event_wait(&events[context], mask, false, K_FOREVER);
}

void cpp_event_clear(uint32_t context, uint32_t mask) { k_event_clear(&events[context], mask); }

// Timers

static void timer_expired(struct k_timer *timer)
{
	rust_task_timer_expired((uint32_t)(uintptr_t)k_timer_user_data_get(timer));
}

static int init_timers(void)
{
	for (uint32_t i = 0; i < TASK_COUNT; ++i) {
		k_timer_init(&timers[i], timer_expired, NULL);
		k_timer_user_data_set(&timers[i], (void *)(uintptr_t)i);
	}
	return 0;
}
SYS_INIT(init_timers, APPLICATION, CONFIG_KERNEL_INIT_PRIORITY_DEFAULT);

void cpp_timer_start_us(uint32_t context, uint32_t microseconds)
{
	k_timer_start(&timers[context], K_USEC(microseconds), K_NO_WAIT);
}

void cpp_timer_stop(uint32_t context) { k_timer_stop(&timers[context]); }

// Kernel

uint32_t cpp_irq_lock(void) { return irq_lock(); }
void cpp_irq_unlock(uint32_t key) { irq_unlock(key); }
bool cpp_is_in_isr(void) { return k_is_in_isr(); }
int32_t cpp_current_priority(void) { return k_thread_priority_get(k_current_get()); }
// k_current_get() may read a stale thread pointer inside the switch hooks.
int32_t cpp_sched_current_priority(void) { return k_thread_priority_get(k_sched_current_thread_query()); }
int32_t cpp_msleep(int32_t milliseconds) { return k_msleep(milliseconds); }

void cpp_reboot_cold(void)
{
	sys_reboot(SYS_REBOOT_COLD);
	for (;;) {
	}
}

// Time

uint64_t cpp_cycle_get_64(void) { return k_cycle_get_64(); }
uint32_t cpp_cyc_to_us_floor32(uint64_t cycles) { return k_cyc_to_us_floor32(cycles); }
uint64_t cpp_cyc_to_us_floor64(uint64_t cycles) { return k_cyc_to_us_floor64(cycles); }
uint32_t cpp_cyc_to_ms_floor32(uint64_t cycles) { return k_cyc_to_ms_floor32(cycles); }
uint64_t cpp_cyc_to_ns_floor64(uint64_t cycles) { return k_cyc_to_ns_floor64(cycles); }
uint32_t cpp_cycles_per_sec(void) { return sys_clock_hw_cycles_per_sec(); }

// Console

static const struct device *const console_dev = DEVICE_DT_GET(DT_CHOSEN(zephyr_console));

int32_t cpp_uart_poll_in(void)
{
	// bspZephyr's getByteFromStdin: an unsigned char initialized to -1 reads 255 when
	// no byte is waiting, which the console input treats as "no input".
	unsigned char c = -1;
	uart_poll_in(console_dev, &c);
	return c;
}

void cpp_uart_poll_out(uint8_t byte) { uart_poll_out(console_dev, byte); }

// CAN

#ifdef CONFIG_CAN
static const struct device *const can_dev = DEVICE_DT_GET(DT_CHOSEN(zephyr_canbus));
static const struct can_filter match_all_filter = {.id = 0, .mask = 0, .flags = 0};

bool cpp_can_device_is_ready(void) { return device_is_ready(can_dev); }
int32_t cpp_can_set_mode_normal(void) { return can_set_mode(can_dev, CAN_MODE_NORMAL); }
int32_t cpp_can_start(void) { return can_start(can_dev); }
int32_t cpp_can_stop(void) { return can_stop(can_dev); }

static void rx_callback(const struct device *dev, struct can_frame *frame, void *user)
{
	ARG_UNUSED(dev);
	rust_can_rx(frame->id, (frame->flags & CAN_FRAME_IDE) != 0, frame->dlc, frame->data, user);
}

int32_t cpp_can_add_rx_filter_all(void)
{
	return can_add_rx_filter(can_dev, rx_callback, NULL, &match_all_filter);
}

void cpp_can_remove_rx_filter(int32_t filter_id) { can_remove_rx_filter(can_dev, filter_id); }

static void tx_callback(const struct device *dev, int error, void *user)
{
	ARG_UNUSED(dev);
	rust_can_tx_done(error, user);
}

int32_t cpp_can_send(uint32_t id, bool extended, uint8_t dlc, const uint8_t *data, void *user)
{
	struct can_frame frame;
	if (dlc > sizeof(frame.data)) {
		return -EINVAL;
	}
	memset(&frame, 0, sizeof(frame));
	frame.id = id;
	frame.flags = extended ? CAN_FRAME_IDE : 0;
	frame.dlc = dlc;
	memcpy(frame.data, data, dlc);
	return can_send(can_dev, &frame, K_NO_WAIT, tx_callback, user);
}

int32_t cpp_can_get_state(int32_t *state, uint8_t *tx_error_count, uint8_t *rx_error_count)
{
	enum can_state can_state_value;
	struct can_bus_err_cnt err_cnt;
	int result = can_get_state(can_dev, &can_state_value, &err_cnt);
	if (result == 0) {
		*state = (int32_t)can_state_value;
		*tx_error_count = err_cnt.tx_err_cnt;
		*rx_error_count = err_cnt.rx_err_cnt;
	}
	return result;
}
#endif

// PWM

#ifdef CONFIG_PWM
static const struct pwm_dt_spec pwm_led0 = PWM_DT_SPEC_GET(DT_ALIAS(pwm_led0));

int32_t cpp_pwm_set_led0(uint32_t period_ns, uint32_t pulse_ns)
{
	return pwm_set_dt(&pwm_led0, period_ns, pulse_ns);
}
#endif

// Tracing hooks: the kernel's weak symbols are overridden only from an application
// object, so they live here and forward to Rust (as TraceHooks.cpp does). Every
// interrupt on this board counts toward ISR group 0, "test".

#define ISR_GROUP_TEST 0

void sys_trace_thread_switched_in_user(void)
{
	if (!application_initialized) {
		return;
	}
	unsigned int key = irq_lock();
	rust_async_enter_task(cpp_sched_current_priority() - 1);
	irq_unlock(key);
}

void sys_trace_thread_switched_out_user(void)
{
	if (!application_initialized) {
		return;
	}
	unsigned int key = irq_lock();
	rust_async_leave_task(cpp_sched_current_priority() - 1);
	irq_unlock(key);
}

void sys_trace_isr_enter_user(void)
{
	if (!application_initialized) {
		return;
	}
	rust_async_enter_isr_group(ISR_GROUP_TEST);
}

void sys_trace_isr_exit_user(void)
{
	if (!application_initialized) {
		return;
	}
	rust_async_leave_isr_group(ISR_GROUP_TEST);
}
