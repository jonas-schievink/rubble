#![cfg_attr(not(feature = "log"), allow(unused))]

use {
    bbqueue::{bbq, BBQueue, Consumer},
    cortex_m::interrupt,
    demo_utils::logging::{BbqLogger, StampedLogger, WriteLogger},
    rubble_nrf5x::timer::StampSource,
};

#[cfg(feature = "log")]
use log::LevelFilter;

type Logger = StampedLogger<StampSource<LogTimer>, BbqLogger>;

type LogTimer = crate::hal::target::TIMER0;

/// Stores the global logger used by the `log` crate.
static mut LOGGER: Option<WriteLogger<Logger>> = None;

#[cfg(feature = "log")]
pub fn init(timer: StampSource<LogTimer>) -> Consumer {
    let (tx, log_sink) = bbq![10000].unwrap().split();
    let logger = StampedLogger::new(BbqLogger::new(tx), timer);

    let log = WriteLogger::new(logger);
    interrupt::free(|_| unsafe {
        // Safe, since we're the only thread and interrupts are off
        LOGGER = Some(log);
        log::set_logger(LOGGER.as_ref().unwrap()).unwrap();
    });
    log::set_max_level(LevelFilter::max());

    log::info!("Logger ready");

    log_sink
}

#[cfg(not(feature = "log"))]
pub fn init(timer: StampSource<LogTimer>) -> Consumer {
    bbq![1].unwrap().split().1
}
