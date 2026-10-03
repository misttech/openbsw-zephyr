// Copyright 2026 Mist Tecnologia LTDA. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

//! The demo's logger, ported from `src/logger/logger.cpp`: the component table, the
//! 8 KB buffered output, and the console composition named `Core0`.

use openbsw_async_zephyr::LockType;
use openbsw_bsp_zephyr::ZephyrStdio;
use openbsw_bsp_zephyr::system_timer::system_time_ns;
use openbsw_console::CONSOLE;
use openbsw_cpp2can::CAN;
use openbsw_docan::DOCAN;
use openbsw_lifecycle::LIFECYCLE;
use openbsw_logger::{
    BufferedLoggerOutput, ComponentConfig, ComponentMapping, DefaultLoggerTime, LoggerComposition,
    MappingInfo,
};
use openbsw_transport::TPROUTER;
use openbsw_uds::UDS;
use openbsw_util::format::{Color, StringAttributes};
use openbsw_util::logger::{Level, LevelInfo, LoggerComponent};

/// The `BSP` component.
pub static BSP: LoggerComponent = LoggerComponent::new();
/// The `COMMON` component.
pub static COMMON: LoggerComponent = LoggerComponent::new();
/// The `DEMO` component (`app/DemoLogger.h`).
pub static DEMO: LoggerComponent = LoggerComponent::new();
/// The `GLOBAL` component, whose level gates all others.
pub static GLOBAL: LoggerComponent = LoggerComponent::new();

const DEFAULT: StringAttributes = StringAttributes::color(Color::DefaultColor);

/// The `LOGGER_COMPONENT_MAPPING_INFO` table of `logger.cpp`, in its order.
static COMPONENT_INFOS: [MappingInfo; 10] = [
    MappingInfo::new(Level::Debug, &BSP, b"BSP", DEFAULT),
    MappingInfo::new(Level::Debug, &COMMON, b"COMMON", DEFAULT),
    MappingInfo::new(Level::Debug, &DEMO, b"DEMO", DEFAULT),
    MappingInfo::new(Level::Debug, &GLOBAL, b"GLOBAL", DEFAULT),
    MappingInfo::new(
        Level::Debug,
        &LIFECYCLE,
        b"LIFECYCLE",
        StringAttributes::color(Color::DarkGray),
    ),
    MappingInfo::new(Level::Debug, &CONSOLE, b"CONSOLE", DEFAULT),
    MappingInfo::new(Level::Debug, &CAN, b"CAN", StringAttributes::color(Color::LightBlue)),
    MappingInfo::new(Level::Debug, &DOCAN, b"DOCAN", StringAttributes::color(Color::LightBlue)),
    MappingInfo::new(Level::Debug, &UDS, b"UDS", StringAttributes::color(Color::LightYellow)),
    MappingInfo::new(
        Level::Debug,
        &TPROUTER,
        b"TPROUTER",
        StringAttributes::color(Color::LightYellow),
    ),
];

static MAPPING: ComponentMapping<10> =
    ComponentMapping::new(&COMPONENT_INFOS, LevelInfo::default_table(), Some(&GLOBAL));
static CONFIG: ComponentConfig<10> = ComponentConfig::new(&MAPPING);
static TIME: DefaultLoggerTime = DefaultLoggerTime::new(&system_time_ns, b"%u");
/// `loggerIntegration/Config.h`: 8 KB of entries of at most 128 bytes.
static BUFFERED: BufferedLoggerOutput<8192, 128, LockType> =
    BufferedLoggerOutput::new(&MAPPING, &TIME);
static STDIO: ZephyrStdio = ZephyrStdio;
static COMPOSITION: LoggerComposition =
    LoggerComposition::new(&BUFFERED, &BUFFERED, &TIME, b"Core0", &STDIO);

/// Start logging.
pub fn init() {
    COMPOSITION.start(|output| CONFIG.start(output));
}

/// Output the next buffered entry, if any.
pub fn run() {
    COMPOSITION.run();
}

/// Output every buffered entry and stop logging.
pub fn flush() {
    COMPOSITION.stop(|| CONFIG.shutdown());
}
