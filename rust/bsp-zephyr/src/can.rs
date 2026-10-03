// Copyright 2026 Mist Tecnologia LTDA. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

//! The CAN controller through Zephyr's CAN API, ported from
//! `can/transceiver/ZephyrCanTransceiver.h` and `ZephyrCanTransceiver.cpp`.
//!
//! Received frames are queued from the driver's callback and handed to the listeners by a
//! runnable on the transceiver's context; frames sent with a listener wait in a short
//! queue and go out one at a time, the driver's completion callback starting the next. A
//! 10 ms cyclic runnable polls the controller for bus-off.

use core::cell::Cell;
use core::ffi::c_void;

use openbsw_async::{ContextType, QueueNode, Runnable, TimeUnit, Timeout};
use openbsw_async_zephyr::{LockType, ModifiableLockType};
use openbsw_cpp2can::filter::Filter;
use openbsw_cpp2can::{
    AbstractCanTransceiver, CAN, CanFrame, CanFrameListener, CanFrameSentListener, CanTransceiver,
    CanTransceiverStateListener, ErrorCode, FilteredCanFrameSentListener, INVALID_FRAME_ID,
    RX_QUEUE_SIZE, State, TransceiverState, can_id,
};
use openbsw_util::{log_debug, log_error, log_warn};

use crate::system_timer::system_time_us32;

/// How often the controller is polled for bus-off, in milliseconds.
const ERROR_POLLING_TIMEOUT: u32 = 10;
/// How many frames sent with a listener can wait (`etl::deque<TxJobWithCallback, 3>`).
const TX_QUEUE_SIZE: usize = 3;

/// A frame sent with a listener, waiting for the controller.
///
/// The C++ job keeps a reference to the caller's frame; this one keeps a copy, so the
/// caller need not keep its frame alive until the listener is called.
#[derive(Clone, Copy)]
struct TxJob {
    listener: &'static dyn CanFrameSentListener,
    frame: CanFrame,
}

/// The received frames waiting for the receive task (`etl::queue<CANFrame, 32>`).
struct RxQueue {
    frames: [Cell<CanFrame>; RX_QUEUE_SIZE],
    head: Cell<usize>,
    len: Cell<usize>,
}

impl RxQueue {
    const fn new() -> Self {
        Self {
            frames: [const { Cell::new(CanFrame::new()) }; RX_QUEUE_SIZE],
            head: Cell::new(0),
            len: Cell::new(0),
        }
    }

    fn is_empty(&self) -> bool {
        self.len.get() == 0
    }

    fn is_full(&self) -> bool {
        self.len.get() == RX_QUEUE_SIZE
    }

    fn push(&self, frame: CanFrame) {
        let index = (self.head.get() + self.len.get()) % RX_QUEUE_SIZE;
        self.frames[index].set(frame);
        self.len.set(self.len.get() + 1);
    }

    fn front(&self) -> CanFrame {
        self.frames[self.head.get()].get()
    }

    fn pop(&self) {
        self.head.set((self.head.get() + 1) % RX_QUEUE_SIZE);
        self.len.set(self.len.get() - 1);
    }
}

/// The frames sent with a listener (`etl::deque<TxJobWithCallback, 3>`).
struct TxQueue {
    jobs: [Cell<Option<TxJob>>; TX_QUEUE_SIZE],
    head: Cell<usize>,
    len: Cell<usize>,
}

impl TxQueue {
    const fn new() -> Self {
        Self {
            jobs: [const { Cell::new(None) }; TX_QUEUE_SIZE],
            head: Cell::new(0),
            len: Cell::new(0),
        }
    }

    fn is_empty(&self) -> bool {
        self.len.get() == 0
    }

    fn is_full(&self) -> bool {
        self.len.get() == TX_QUEUE_SIZE
    }

    fn push_back(&self, job: TxJob) {
        let index = (self.head.get() + self.len.get()) % TX_QUEUE_SIZE;
        self.jobs[index].set(Some(job));
        self.len.set(self.len.get() + 1);
    }

    fn front(&self) -> Option<TxJob> {
        self.jobs[self.head.get()].get()
    }

    fn pop_front(&self) {
        self.jobs[self.head.get()].set(None);
        self.head.set((self.head.get() + 1) % TX_QUEUE_SIZE);
        self.len.set(self.len.get() - 1);
    }

    fn clear(&self) {
        for job in &self.jobs {
            job.set(None);
        }
        self.head.set(0);
        self.len.set(0);
    }
}

/// The runnable that drains the receive queue: the port of `_receiveTask`.
struct ReceiveTask {
    owner: &'static ZephyrCanTransceiver,
    node: QueueNode<dyn Runnable>,
}

impl Runnable for ReceiveTask {
    fn execute(&self) {
        self.owner.receive_task();
    }

    fn node(&self) -> &QueueNode<dyn Runnable> {
        &self.node
    }
}

/// The runnable that polls the bus state: the port of `_cyclicTask`.
struct CyclicTask {
    owner: &'static ZephyrCanTransceiver,
    node: QueueNode<dyn Runnable>,
}

impl Runnable for CyclicTask {
    fn execute(&self) {
        self.owner.cyclic_task();
    }

    fn node(&self) -> &QueueNode<dyn Runnable> {
        &self.node
    }
}

/// The CAN transceiver over the controller the Device Tree chooses (`zephyr,canbus`).
pub struct ZephyrCanTransceiver {
    base: AbstractCanTransceiver<LockType>,
    rx_queue: RxQueue,
    rx_filter_id: Cell<i32>,
    tx_offline_errors: Cell<u16>,
    overrun_count: Cell<u32>,
    frames_sent_count: Cell<u32>,
    tx_queue: TxQueue,
    context: ContextType,
    bus_name: &'static [u8],
    cyclic_task: CyclicTask,
    receive_task: ReceiveTask,
    cyclic_task_timeout: Timeout,
}

// SAFETY: the queues and counters are changed under the platform lock, the state on the
// transceiver's own context, as the C++ transceiver does with the same lock.
unsafe impl Sync for ZephyrCanTransceiver {}

impl ZephyrCanTransceiver {
    /// A closed transceiver whose tasks run on `context`, on bus `bus_id` named
    /// `bus_name` in the log (`BusIdTraits::getName`). `this` is the transceiver's own
    /// `'static` address, which its runnables keep (the C++ members take `*this`).
    pub const fn new(
        this: &'static ZephyrCanTransceiver,
        context: ContextType,
        bus_id: u8,
        bus_name: &'static [u8],
    ) -> Self {
        Self {
            base: AbstractCanTransceiver::new(bus_id, &system_time_us32),
            rx_queue: RxQueue::new(),
            rx_filter_id: Cell::new(0),
            tx_offline_errors: Cell::new(0),
            overrun_count: Cell::new(0),
            frames_sent_count: Cell::new(0),
            tx_queue: TxQueue::new(),
            context,
            bus_name,
            cyclic_task: CyclicTask { owner: this, node: QueueNode::new() },
            receive_task: ReceiveTask { owner: this, node: QueueNode::new() },
            cyclic_task_timeout: Timeout::new(),
        }
    }

    /// How many frames were dropped because a queue was full.
    pub fn overrun_count(&self) -> u32 {
        self.overrun_count.get()
    }

    /// How many frames the controller took.
    pub fn frames_sent_count(&self) -> u32 {
        self.frames_sent_count.get()
    }

    /// How many writes were refused while closed.
    pub fn tx_offline_errors(&self) -> u16 {
        self.tx_offline_errors.get()
    }

    /// The first frame id: this transceiver has none (the C++ asserts).
    pub fn first_frame_id(&self) -> u16 {
        INVALID_FRAME_ID
    }

    fn this(&self) -> &'static ZephyrCanTransceiver {
        self.receive_task.owner
    }

    fn send(&self, frame: &CanFrame, user: *mut c_void) -> i32 {
        zephyr_ffi::can_send(
            can_id::raw_id(frame.id()),
            can_id::is_extended(frame.id()),
            frame.payload(),
            user,
        )
    }

    fn write_impl(
        &self,
        frame: &mut CanFrame,
        listener: Option<&'static dyn CanFrameSentListener>,
    ) -> ErrorCode {
        log_debug!(CAN, b"write()");
        if self.base.state() == State::Muted {
            log_warn!(CAN, b"Write Id 0x%x to muted %s", frame.id(), self.bus_name);
            return ErrorCode::IllegalState;
        }
        if self.base.state() == State::Closed {
            self.tx_offline_errors.set(self.tx_offline_errors.get().wrapping_add(1));
            return ErrorCode::TxOffline;
        }
        let mut mlock = ModifiableLockType::new();
        let Some(listener) = listener else {
            let result = self.send(frame, core::ptr::null_mut());
            if result == -zephyr_ffi::EAGAIN {
                self.overrun_count.set(self.overrun_count.get().wrapping_add(1));
                mlock.unlock();
                self.base.notify_sent_listeners(frame);
                return ErrorCode::TxHwQueueFull;
            }
            if result == 0 {
                self.frames_sent_count.set(self.frames_sent_count.get().wrapping_add(1));
                mlock.unlock();
                self.base.notify_sent_listeners(frame);
                return ErrorCode::Ok;
            }
            mlock.unlock();
            self.base.notify_sent_listeners(frame);
            return ErrorCode::TxFail;
        };
        if self.tx_queue.is_full() {
            self.overrun_count.set(self.overrun_count.get().wrapping_add(1));
            mlock.unlock();
            self.base.notify_sent_listeners(frame);
            return ErrorCode::TxHwQueueFull;
        }
        let was_empty = self.tx_queue.is_empty();
        self.tx_queue.push_back(TxJob { listener, frame: *frame });
        if !was_empty {
            return ErrorCode::Ok;
        }
        let result = self.send(frame, self.user());
        let status = if result == -zephyr_ffi::EAGAIN {
            ErrorCode::TxHwQueueFull
        } else if result != 0 {
            ErrorCode::TxFail
        } else {
            return ErrorCode::Ok;
        };
        self.tx_queue.pop_front();
        mlock.unlock();
        self.base.notify_sent_listeners(frame);
        status
    }

    /// The controller took the frame at the front of the queue: tell its listener and
    /// send the next one.
    fn can_frame_sent_callback(&self, _error: i32) {
        let mut mlock = ModifiableLockType::new();
        self.frames_sent_count.set(self.frames_sent_count.get().wrapping_add(1));
        let Some(job) = self.tx_queue.front() else {
            return;
        };
        self.tx_queue.pop_front();
        let mut send_again = false;
        if !self.tx_queue.is_empty() {
            if matches!(self.base.state(), State::Open | State::Initialized) {
                send_again = true;
            } else {
                self.tx_queue.clear();
            }
        }
        mlock.unlock();
        let mut frame = job.frame;
        job.listener.can_frame_sent(&frame);
        self.base.notify_sent_listeners(&mut frame);
        if send_again {
            mlock.lock();
            let Some(next) = self.tx_queue.front() else {
                return;
            };
            if self.send(&next.frame, self.user()) == 0 {
                return;
            }
            let mut frame = next.frame;
            self.tx_queue.clear();
            mlock.unlock();
            self.base.notify_sent_listeners(&mut frame);
        }
    }

    /// Hand every queued frame to the listeners, releasing the lock around each.
    fn receive_task(&self) {
        let mut mlock = ModifiableLockType::new();
        while !self.rx_queue.is_empty() {
            let frame = self.rx_queue.front();
            mlock.unlock();
            self.base.notify_listeners(&frame);
            mlock.lock();
            self.rx_queue.pop();
        }
    }

    /// Poll the controller: report bus-off and recovery to the state listener.
    fn cyclic_task(&self) {
        let bus_off = match zephyr_ffi::can_get_state() {
            Ok((state, _tx_errors, _rx_errors)) => state == zephyr_ffi::CAN_STATE_BUS_OFF,
            Err(_) => {
                log_warn!(CAN, b"Can not get CAN state, assume BUS_OFF for %s", self.bus_name);
                true
            }
        };
        let wanted = if bus_off { TransceiverState::BusOff } else { TransceiverState::Active };
        if self.base.can_transceiver_state() != wanted {
            self.base.set_transceiver_state(wanted);
            self.base.notify_state_listener_with_state(self, wanted);
        }
    }

    /// Queue a received frame if the queue has room and the listeners' filters accept
    /// its id; extended ids bypass the filter, as in C++.
    fn enqueue_rx_frame(&self, id: u32, data: &[u8], extended: bool) -> bool {
        let _lock = <LockType as openbsw_async::Lock>::lock();
        if self.rx_queue.is_full() {
            return false;
        }
        if !extended && !self.base.filter().matches(id) {
            return false;
        }
        let mut frame = CanFrame::new();
        frame.set_timestamp(system_time_us32());
        frame.set_id(can_id::id(id, extended));
        let length = data.len().min(usize::from(CanFrame::MAX_FRAME_LENGTH));
        frame.set_payload(&data[..length]);
        self.rx_queue.push(frame);
        true
    }

    fn can_frame_received_callback(&self, id: u32, data: &[u8], extended: bool) {
        if self.enqueue_rx_frame(id, data, extended) {
            openbsw_async::execute(self.context, &self.this().receive_task);
        }
    }

    fn user(&self) -> *mut c_void {
        (self as *const Self).cast_mut().cast()
    }
}

/// The driver received a frame; called by the shim's receive callback with the `user`
/// pointer `open` registered: the transceiver.
#[unsafe(no_mangle)]
pub extern "C" fn rust_can_rx(
    id: u32,
    extended: bool,
    length: u8,
    data: *const u8,
    user: *mut c_void,
) {
    if user.is_null() || data.is_null() {
        return;
    }
    // SAFETY: `user` is the address `open` registered, a `'static` transceiver; the shim
    // passes the frame's data, valid for `length` bytes during the call.
    let (transceiver, data) = unsafe {
        (
            &*user.cast_const().cast::<ZephyrCanTransceiver>(),
            core::slice::from_raw_parts(data, usize::from(length)),
        )
    };
    transceiver.can_frame_received_callback(id, data, extended);
}

/// The driver finished sending a frame; called by the shim's transmit callback with the
/// `user` pointer `write` passed: the transceiver for a frame with a listener, null
/// otherwise.
#[unsafe(no_mangle)]
pub extern "C" fn rust_can_tx_done(error: i32, user: *mut c_void) {
    if user.is_null() {
        return;
    }
    // SAFETY: `user` is the address `write` passed, a `'static` transceiver.
    let transceiver = unsafe { &*user.cast_const().cast::<ZephyrCanTransceiver>() };
    transceiver.can_frame_sent_callback(error);
}

impl CanTransceiver for ZephyrCanTransceiver {
    fn init(&self) -> ErrorCode {
        if self.base.state() != State::Closed {
            return ErrorCode::IllegalState;
        }
        if !zephyr_ffi::can_device_is_ready() {
            log_error!(CAN, b"Device not ready for %s", self.bus_name);
            return ErrorCode::InitFailed;
        }
        if zephyr_ffi::can_set_mode_normal() != 0 {
            log_error!(CAN, b"Could not set normal mode for %s", self.bus_name);
            return ErrorCode::InitFailed;
        }
        log_debug!(CAN, b"init()");
        self.base.set_state(State::Initialized);
        ErrorCode::Ok
    }

    fn shutdown(&self) {
        let _ = self.close();
        self.this().cyclic_task_timeout.cancel();
    }

    /// Opening on a wake-up frame is not supported here (the C++ asserts).
    fn open_with(&self, _frame: &CanFrame) -> ErrorCode {
        ErrorCode::IllegalState
    }

    fn open(&self) -> ErrorCode {
        if !matches!(self.base.state(), State::Initialized | State::Closed) {
            return ErrorCode::IllegalState;
        }
        let filter_id = zephyr_ffi::can_add_rx_filter_all(self.user());
        self.rx_filter_id.set(filter_id);
        if filter_id == -zephyr_ffi::ENOSPC || filter_id == -zephyr_ffi::ENOTSUP {
            log_error!(CAN, b"Can not add rx filter for %s", self.bus_name);
            return ErrorCode::InitFailed;
        }
        if zephyr_ffi::can_start() == 0 {
            self.base.set_state(State::Open);
            let this = self.this();
            openbsw_async::schedule_at_fixed_rate(
                self.context,
                &this.cyclic_task,
                &this.cyclic_task_timeout,
                ERROR_POLLING_TIMEOUT,
                TimeUnit::Milliseconds,
            );
            log_debug!(CAN, b"open()");
            ErrorCode::Ok
        } else {
            log_error!(CAN, b"Can not start CAN for %s", self.bus_name);
            ErrorCode::IllegalState
        }
    }

    fn close(&self) -> ErrorCode {
        if !matches!(self.base.state(), State::Open | State::Muted) {
            return ErrorCode::IllegalState;
        }
        if zephyr_ffi::can_stop() != 0 {
            log_error!(CAN, b"Can not stop CAN for %s", self.bus_name);
            return ErrorCode::IllegalState;
        }
        zephyr_ffi::can_remove_rx_filter(self.rx_filter_id.get());
        self.this().cyclic_task_timeout.cancel();
        self.base.set_state(State::Closed);
        {
            let _lock = <LockType as openbsw_async::Lock>::lock();
            self.tx_queue.clear();
        }
        ErrorCode::Ok
    }

    fn mute(&self) -> ErrorCode {
        if self.base.state() != State::Open {
            return ErrorCode::IllegalState;
        }
        {
            let _lock = <LockType as openbsw_async::Lock>::lock();
            self.tx_queue.clear();
        }
        self.base.set_state(State::Muted);
        ErrorCode::Ok
    }

    fn unmute(&self) -> ErrorCode {
        if self.base.state() != State::Muted {
            return ErrorCode::IllegalState;
        }
        self.base.set_state(State::Open);
        ErrorCode::Ok
    }

    fn state(&self) -> State {
        self.base.state()
    }

    fn baudrate(&self) -> u32 {
        zephyr_ffi::can_bitrate()
    }

    fn hw_queue_timeout(&self) -> u16 {
        ((53 + 64) * 1000 / self.baudrate()) as u16
    }

    fn write(&self, frame: &mut CanFrame) -> ErrorCode {
        self.write_impl(frame, None)
    }

    fn write_with_listener(
        &self,
        frame: &mut CanFrame,
        listener: &'static dyn CanFrameSentListener,
    ) -> ErrorCode {
        self.write_impl(frame, Some(listener))
    }

    fn add_can_frame_listener(&self, listener: &'static dyn CanFrameListener) {
        self.base.add_can_frame_listener(listener);
    }

    fn add_vip_can_frame_listener(&self, listener: &'static dyn CanFrameListener) {
        self.base.add_vip_can_frame_listener(listener);
    }

    fn remove_can_frame_listener(&self, listener: &dyn CanFrameListener) {
        self.base.remove_can_frame_listener(listener);
    }

    fn bus_id(&self) -> u8 {
        self.base.bus_id()
    }

    fn add_can_frame_sent_listener(&self, listener: &'static dyn FilteredCanFrameSentListener) {
        self.base.add_can_frame_sent_listener(listener);
    }

    fn remove_can_frame_sent_listener(&self, listener: &dyn FilteredCanFrameSentListener) {
        self.base.remove_can_frame_sent_listener(listener);
    }

    fn can_transceiver_state(&self) -> TransceiverState {
        self.base.can_transceiver_state()
    }

    fn set_state_listener(&self, listener: &'static dyn CanTransceiverStateListener) {
        self.base.set_state_listener(listener);
    }

    fn remove_state_listener(&self) {
        self.base.remove_state_listener();
    }
}

#[cfg(test)]
mod tests {
    extern crate std;

    use std::cell::RefCell;
    use std::sync::Mutex;
    use std::vec::Vec;

    use openbsw_async::mock::MockAsync;
    use openbsw_cpp2can::ListenerNode;
    use openbsw_cpp2can::filter::{Filter, IntervalFilter};
    use zephyr_ffi::host::Hooks;

    use super::*;

    /// The hooks and the async binding are process-wide, so the tests run one at a time.
    static SERIAL: Mutex<()> = Mutex::new(());

    #[derive(Debug, Clone, PartialEq, Eq)]
    enum Call {
        SetModeNormal,
        Start,
        Stop,
        AddFilter(bool),
        RemoveFilter(i32),
        Send(u32, bool, Vec<u8>, bool),
    }

    /// Zephyr's CAN API as the tests see it.
    struct Driver {
        calls: Mutex<Vec<Call>>,
        send_result: Mutex<i32>,
        state: Mutex<Result<i32, i32>>,
    }

    static DRIVER: Driver = Driver {
        calls: Mutex::new(Vec::new()),
        send_result: Mutex::new(0),
        state: Mutex::new(Ok(0)),
    };

    impl Driver {
        fn record(&self, call: Call) {
            self.calls.lock().unwrap().push(call);
        }
        fn take(&self) -> Vec<Call> {
            core::mem::take(&mut *self.calls.lock().unwrap())
        }
    }

    impl Hooks for Driver {
        fn can_set_mode_normal(&self) -> i32 {
            self.record(Call::SetModeNormal);
            0
        }
        fn can_start(&self) -> i32 {
            self.record(Call::Start);
            0
        }
        fn can_stop(&self) -> i32 {
            self.record(Call::Stop);
            0
        }
        fn can_add_rx_filter_all(&self, user: *mut c_void) -> i32 {
            self.record(Call::AddFilter(!user.is_null()));
            7
        }
        fn can_remove_rx_filter(&self, filter_id: i32) {
            self.record(Call::RemoveFilter(filter_id));
        }
        fn can_send(&self, id: u32, extended: bool, data: &[u8], user: *mut c_void) -> i32 {
            self.record(Call::Send(id, extended, data.to_vec(), !user.is_null()));
            *self.send_result.lock().unwrap()
        }
        fn can_get_state(&self) -> Result<(i32, u8, u8), i32> {
            self.state.lock().unwrap().map(|state| (state, 0, 0))
        }
        fn cycle_get_64(&self) -> u64 {
            4242
        }
    }

    const CAN_CONTEXT: ContextType = 1;
    static MOCK_ASYNC: MockAsync<2> = MockAsync::new([b"sysadmin" as &[u8], b"can"]);
    static TRANSCEIVER: ZephyrCanTransceiver =
        ZephyrCanTransceiver::new(&TRANSCEIVER, CAN_CONTEXT, 2, b"CAN_0");

    fn start() -> std::sync::MutexGuard<'static, ()> {
        let guard = SERIAL.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        zephyr_ffi::host::install(&DRIVER);
        openbsw_async::set_binding(&MOCK_ASYNC);
        DRIVER.take();
        *DRIVER.send_result.lock().unwrap() = 0;
        *DRIVER.state.lock().unwrap() = Ok(0);
        // Back to closed, whatever the previous test left.
        let _ = TRANSCEIVER.close();
        TRANSCEIVER.base.set_state(State::Closed);
        TRANSCEIVER.base.remove_state_listener();
        DRIVER.take();
        guard
    }

    struct Receiver {
        filter: IntervalFilter,
        frames: RefCell<Vec<CanFrame>>,
        node: ListenerNode,
    }
    // SAFETY: single-threaded test object.
    unsafe impl Sync for Receiver {}
    impl CanFrameListener for Receiver {
        fn frame_received(&self, frame: &CanFrame) {
            self.frames.borrow_mut().push(*frame);
        }
        fn filter(&self) -> &dyn Filter {
            &self.filter
        }
        fn node(&self) -> &ListenerNode {
            &self.node
        }
    }
    static RECEIVER: Receiver = Receiver {
        filter: IntervalFilter::with_range(0x100, 0x1FF),
        frames: RefCell::new(Vec::new()),
        node: ListenerNode::new(),
    };

    struct SentRecorder(RefCell<Vec<CanFrame>>);
    // SAFETY: single-threaded test object.
    unsafe impl Sync for SentRecorder {}
    impl CanFrameSentListener for SentRecorder {
        fn can_frame_sent(&self, frame: &CanFrame) {
            self.0.borrow_mut().push(*frame);
        }
    }
    static SENT: SentRecorder = SentRecorder(RefCell::new(Vec::new()));

    struct StateRecorder(RefCell<Vec<TransceiverState>>);
    // SAFETY: single-threaded test object.
    unsafe impl Sync for StateRecorder {}
    impl CanTransceiverStateListener for StateRecorder {
        fn can_transceiver_state_changed(
            &self,
            _transceiver: &dyn CanTransceiver,
            state: TransceiverState,
        ) {
            self.0.borrow_mut().push(state);
        }
        fn phy_error_occurred(&self, _transceiver: &dyn CanTransceiver) {}
    }
    static STATES: StateRecorder = StateRecorder(RefCell::new(Vec::new()));

    fn open() {
        assert_eq!(TRANSCEIVER.init(), ErrorCode::Ok);
        assert_eq!(TRANSCEIVER.open(), ErrorCode::Ok);
        assert_eq!(DRIVER.take(), [Call::SetModeNormal, Call::AddFilter(true), Call::Start]);
    }

    #[test]
    fn init_and_open_follow_the_state_machine() {
        let _serial = start();
        assert_eq!(TRANSCEIVER.state(), State::Closed);
        assert_eq!(TRANSCEIVER.init(), ErrorCode::Ok);
        assert_eq!(TRANSCEIVER.state(), State::Initialized);
        assert_eq!(TRANSCEIVER.init(), ErrorCode::IllegalState);
        assert_eq!(TRANSCEIVER.open(), ErrorCode::Ok);
        assert_eq!(TRANSCEIVER.state(), State::Open);
        assert!(MOCK_ASYNC.is_scheduled(&TRANSCEIVER.cyclic_task_timeout));
        assert_eq!(TRANSCEIVER.open(), ErrorCode::IllegalState);
        assert_eq!(TRANSCEIVER.mute(), ErrorCode::Ok);
        assert_eq!(TRANSCEIVER.state(), State::Muted);
        assert_eq!(TRANSCEIVER.mute(), ErrorCode::IllegalState);
        assert_eq!(TRANSCEIVER.unmute(), ErrorCode::Ok);
        assert_eq!(TRANSCEIVER.unmute(), ErrorCode::IllegalState);
        assert_eq!(TRANSCEIVER.close(), ErrorCode::Ok);
        assert_eq!(TRANSCEIVER.state(), State::Closed);
        assert!(!MOCK_ASYNC.is_scheduled(&TRANSCEIVER.cyclic_task_timeout));
        assert_eq!(TRANSCEIVER.close(), ErrorCode::IllegalState);
        assert_eq!(
            DRIVER.take(),
            [
                Call::SetModeNormal,
                Call::AddFilter(true),
                Call::Start,
                Call::Stop,
                Call::RemoveFilter(7)
            ]
        );
        assert_eq!(TRANSCEIVER.baudrate(), 500_000);
        assert_eq!(TRANSCEIVER.hw_queue_timeout(), 0);
        assert_eq!(TRANSCEIVER.bus_id(), 2);
    }

    #[test]
    fn write_without_listener_goes_straight_to_the_driver() {
        let _serial = start();
        let mut frame = CanFrame::from_payload(0x558, &[0, 0, 0, 1]);
        assert_eq!(TRANSCEIVER.write(&mut frame), ErrorCode::TxOffline);
        assert_eq!(TRANSCEIVER.tx_offline_errors(), 1);
        open();
        let before = TRANSCEIVER.frames_sent_count();
        assert_eq!(TRANSCEIVER.write(&mut frame), ErrorCode::Ok);
        assert_eq!(TRANSCEIVER.frames_sent_count(), before + 1);
        assert_eq!(DRIVER.take(), [Call::Send(0x558, false, std::vec![0, 0, 0, 1], false)]);
        let mut extended = CanFrame::from_raw_id(0x1234567, &[9], true);
        assert_eq!(TRANSCEIVER.write(&mut extended), ErrorCode::Ok);
        assert_eq!(DRIVER.take(), [Call::Send(0x1234567, true, std::vec![9], false)]);
        *DRIVER.send_result.lock().unwrap() = -zephyr_ffi::EAGAIN;
        let overruns = TRANSCEIVER.overrun_count();
        assert_eq!(TRANSCEIVER.write(&mut frame), ErrorCode::TxHwQueueFull);
        assert_eq!(TRANSCEIVER.overrun_count(), overruns + 1);
        *DRIVER.send_result.lock().unwrap() = -5;
        assert_eq!(TRANSCEIVER.write(&mut frame), ErrorCode::TxFail);
        assert_eq!(TRANSCEIVER.mute(), ErrorCode::Ok);
        assert_eq!(TRANSCEIVER.write(&mut frame), ErrorCode::IllegalState);
    }

    #[test]
    fn write_with_listener_queues_and_sends_one_at_a_time() {
        let _serial = start();
        open();
        SENT.0.borrow_mut().clear();
        let mut frame1 = CanFrame::from_payload(0x100, &[1]);
        let mut frame2 = CanFrame::from_payload(0x101, &[2]);
        let mut frame3 = CanFrame::from_payload(0x102, &[3]);
        let mut frame4 = CanFrame::from_payload(0x103, &[4]);
        assert_eq!(TRANSCEIVER.write_with_listener(&mut frame1, &SENT), ErrorCode::Ok);
        assert_eq!(DRIVER.take(), [Call::Send(0x100, false, std::vec![1], true)]);
        // The next ones wait for the first to complete.
        assert_eq!(TRANSCEIVER.write_with_listener(&mut frame2, &SENT), ErrorCode::Ok);
        assert_eq!(TRANSCEIVER.write_with_listener(&mut frame3, &SENT), ErrorCode::Ok);
        assert_eq!(DRIVER.take(), []);
        let overruns = TRANSCEIVER.overrun_count();
        assert_eq!(TRANSCEIVER.write_with_listener(&mut frame4, &SENT), ErrorCode::TxHwQueueFull);
        assert_eq!(TRANSCEIVER.overrun_count(), overruns + 1);
        // Completion tells the listener and sends the next.
        rust_can_tx_done(0, TRANSCEIVER.user());
        assert_eq!(SENT.0.borrow().as_slice(), [frame1]);
        assert_eq!(DRIVER.take(), [Call::Send(0x101, false, std::vec![2], true)]);
        rust_can_tx_done(0, TRANSCEIVER.user());
        assert_eq!(SENT.0.borrow().as_slice(), [frame1, frame2]);
        assert_eq!(DRIVER.take(), [Call::Send(0x102, false, std::vec![3], true)]);
        rust_can_tx_done(0, TRANSCEIVER.user());
        assert_eq!(SENT.0.borrow().as_slice(), [frame1, frame2, frame3]);
        assert_eq!(DRIVER.take(), []);
        // A null user is a frame sent without a listener: nothing to do.
        rust_can_tx_done(0, core::ptr::null_mut());
        // A refused first send drops the job and reports it.
        *DRIVER.send_result.lock().unwrap() = -zephyr_ffi::EAGAIN;
        assert_eq!(TRANSCEIVER.write_with_listener(&mut frame1, &SENT), ErrorCode::TxHwQueueFull);
        assert!(TRANSCEIVER.tx_queue.is_empty());
    }

    #[test]
    fn received_frames_reach_the_listeners_on_the_can_context() {
        let _serial = start();
        open();
        RECEIVER.frames.borrow_mut().clear();
        TRANSCEIVER.add_can_frame_listener(&RECEIVER);
        let data = [0xAA, 0xBB, 0xCC];
        // An id no listener wants is dropped at the driver callback.
        rust_can_rx(0x555, false, 3, data.as_ptr(), TRANSCEIVER.user());
        assert!(!MOCK_ASYNC.has_runnable(CAN_CONTEXT));
        rust_can_rx(0x123, false, 3, data.as_ptr(), TRANSCEIVER.user());
        assert!(MOCK_ASYNC.has_runnable(CAN_CONTEXT));
        // Extended ids bypass the base-id filter; this one is then rejected by the
        // listener's own filter.
        rust_can_rx(0x1FFFFFFF, true, 1, data.as_ptr(), TRANSCEIVER.user());
        // A null user or data pointer is ignored.
        rust_can_rx(0x123, false, 3, data.as_ptr(), core::ptr::null_mut());
        rust_can_rx(0x123, false, 3, core::ptr::null(), TRANSCEIVER.user());
        MOCK_ASYNC.run_runnables(CAN_CONTEXT);
        let frames = core::mem::take(&mut *RECEIVER.frames.borrow_mut());
        assert_eq!(frames, [CanFrame::from_payload(0x123, &data)]);
        assert_eq!(frames[0].timestamp(), 4242);
        assert!(TRANSCEIVER.rx_queue.is_empty());
        TRANSCEIVER.remove_can_frame_listener(&RECEIVER);
    }

    #[test]
    fn the_receive_queue_drops_frames_when_full() {
        let _serial = start();
        open();
        RECEIVER.frames.borrow_mut().clear();
        TRANSCEIVER.add_can_frame_listener(&RECEIVER);
        let data = [1];
        for _ in 0..RX_QUEUE_SIZE + 5 {
            rust_can_rx(0x100, false, 1, data.as_ptr(), TRANSCEIVER.user());
        }
        MOCK_ASYNC.run_runnables(CAN_CONTEXT);
        assert_eq!(RECEIVER.frames.borrow().len(), RX_QUEUE_SIZE);
        TRANSCEIVER.remove_can_frame_listener(&RECEIVER);
    }

    #[test]
    fn the_cyclic_task_reports_bus_off_and_recovery() {
        let _serial = start();
        STATES.0.borrow_mut().clear();
        TRANSCEIVER.set_state_listener(&STATES);
        open();
        TRANSCEIVER.cyclic_task();
        assert!(STATES.0.borrow().is_empty());
        *DRIVER.state.lock().unwrap() = Ok(zephyr_ffi::CAN_STATE_BUS_OFF);
        TRANSCEIVER.cyclic_task();
        TRANSCEIVER.cyclic_task();
        assert_eq!(STATES.0.borrow().as_slice(), [TransceiverState::BusOff]);
        assert_eq!(TRANSCEIVER.can_transceiver_state(), TransceiverState::BusOff);
        *DRIVER.state.lock().unwrap() = Ok(0);
        TRANSCEIVER.cyclic_task();
        assert_eq!(
            STATES.0.borrow().as_slice(),
            [TransceiverState::BusOff, TransceiverState::Active]
        );
        // A failed query counts as bus-off.
        *DRIVER.state.lock().unwrap() = Err(-5);
        TRANSCEIVER.cyclic_task();
        assert_eq!(STATES.0.borrow().len(), 3);
        assert_eq!(TRANSCEIVER.can_transceiver_state(), TransceiverState::BusOff);
        TRANSCEIVER.remove_state_listener();
        *DRIVER.state.lock().unwrap() = Ok(0);
        TRANSCEIVER.cyclic_task();
    }
}
