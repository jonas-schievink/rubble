#![cfg_attr(not(feature = "log"), allow(unused))]

use bbqueue::{BBBuffer, ConstBBBuffer, Consumer};
use cortex_m::interrupt;
use demo_utils::logging::{BbqLogger, StampedLogger, WriteLogger};
use rubble_nrf5x::timer::StampSource;

#[cfg(feature = "log")]
pub(crate) use bbqueue::consts::U10000 as BufferSize;

#[cfg(not(feature = "log"))]
pub(crate) use bbqueue::consts::U1 as BufferSize;

#[cfg(feature = "log")]
use log::LevelFilter;

type Logger = StampedLogger<StampSource<LogTimer>, BbqLogger<'static, BufferSize>>;

type LogTimer = crate::hal::pac::TIMER0;

/// Stores the global logger used by the `log` crate.
static mut LOGGER: Option<WriteLogger<Logger>> = None;

/// Stores the global BBBuffer for the log queue.
static BUFFER: BBBuffer<BufferSize> = BBBuffer(ConstBBBuffer::new());

#[cfg(feature = "log")]
pub fn init(timer: StampSource<LogTimer>) -> Consumer<'static, BufferSize> {
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
pub fn init(timer: StampSource<LogTimer>) -> Consumer<'static, BufferSize> {
    BUFFER.try_split().unwrap().1
}
