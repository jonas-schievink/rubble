#![cfg_attr(not(feature = "log"), allow(unused))]

use bbqueue::{BBBuffer, Consumer};
use cortex_m::interrupt;
use demo_utils::logging::{BbqLogger, StampedLogger, WriteLogger};
use rubble_nrf5x::timer::StampSource;

#[cfg(feature = "log")]
pub(crate) const BUFFER_SIZE: usize = 10000;

#[cfg(not(feature = "log"))]
pub(crate) const BUFFER_SIZE: usize = 1;

#[cfg(feature = "log")]
use log::LevelFilter;

type Logger = StampedLogger<StampSource<LogTimer>, BbqLogger<'static, BUFFER_SIZE>>;

type LogTimer = crate::hal::pac::TIMER0;

/// Stores the global logger used by the `log` crate.
static mut LOGGER: Option<WriteLogger<Logger>> = None;

/// Stores the global BBBuffer for the log queue.
static BUFFER: BBBuffer<BUFFER_SIZE> = BBBuffer::<BUFFER_SIZE>::new();

#[cfg(feature = "log")]
pub fn init(timer: StampSource<LogTimer>) -> Consumer<'static, { BUFFER_SIZE }> {
    let (tx, log_sink) = BUFFER.try_split().unwrap();
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
pub fn init(timer: StampSource<LogTimer>) -> Consumer<'static, { BUFFER_SIZE }> {
    BUFFER.try_split().unwrap().1
}
