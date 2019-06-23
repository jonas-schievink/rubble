#![cfg_attr(not(feature = "log"), allow(unused))]

use {
    bbqueue::{bbq, BBQueue, Consumer, Producer},
    core::{cell::RefCell, fmt},
    cortex_m::interrupt::{self, Mutex},
    nrf52810_hal::nrf52810_pac as pac,
    rubble::time::Timer,
    rubble_nrf52::timer::StampSource,
};

#[cfg(feature = "log")]
use log::{LevelFilter, Log, Metadata, Record};

/// A `fmt::Write` adapter that prints a timestamp before each line.
pub struct StampedLogger<T: Timer, L: fmt::Write> {
    timer: T,
    inner: L,
}

impl<T: Timer, L: fmt::Write> StampedLogger<T, L> {
    /// Creates a new `StampedLogger` that will print to `inner` and obtains timestamps using
    /// `timer`.
    pub fn new(inner: L, timer: T) -> Self {
        Self { inner, timer }
    }
}

impl<T: Timer, L: fmt::Write> fmt::Write for StampedLogger<T, L> {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        for (i, line) in s.split('\n').enumerate() {
            if i != 0 {
                write!(self.inner, "\n{} - ", self.timer.now())?;
            }

            self.inner.write_str(line)?;
        }

        Ok(())
    }
}

/// A `fmt::Write` sink that writes to a `BBQueue`.
///
/// The sink will panic when the `BBQueue` doesn't have enough space to the data. This is to ensure
/// that we never block or drop data.
pub struct BbqLogger {
    p: Producer,
}

impl BbqLogger {
    pub fn new(p: Producer) -> Self {
        Self { p }
    }
}

impl fmt::Write for BbqLogger {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        let mut bytes = s.as_bytes();

        while !bytes.is_empty() {
            let mut grant = match self.p.grant_max(bytes.len()) {
                Ok(grant) => grant,
                Err(_) => {
                    let max_len = self
                        .p
                        .grant_max(usize::max_value())
                        .map(|mut g| g.buf().len())
                        .unwrap_or(0);
                    panic!(
                        "log buffer overflow: failed to grant {} Bytes ({} available)",
                        bytes.len(),
                        max_len
                    );
                }
            };
            let size = grant.buf().len();
            grant.buf().copy_from_slice(&bytes[..size]);
            bytes = &bytes[size..];
            self.p.commit(size, grant);
        }

        Ok(())
    }
}

/// Wraps a `fmt::Write` implementor and forwards the `log` crates logging macros to it.
///
/// The inner `fmt::Write` is made `Sync` by wrapping it in a `Mutex` from the `cortex_m` crate.
pub struct WriteLogger<W: fmt::Write + Send> {
    writer: Mutex<RefCell<W>>,
}

impl<W: fmt::Write + Send> WriteLogger<W> {
    pub fn new(writer: W) -> Self {
        Self {
            writer: Mutex::new(RefCell::new(writer)),
        }
    }
}

#[cfg(feature = "log")]
impl<W: fmt::Write + Send> Log for WriteLogger<W> {
    fn enabled(&self, _metadata: &Metadata<'_>) -> bool {
        true
    }

    fn log(&self, record: &Record<'_>) {
        if self.enabled(record.metadata()) {
            interrupt::free(|cs| {
                let mut writer = self.writer.borrow(cs).borrow_mut();
                writeln!(writer, "{} - {}", record.level(), record.args()).unwrap();
            })
        }
    }

    fn flush(&self) {}
}

type Logger = StampedLogger<StampSource<LogTimer>, BbqLogger>;

type LogTimer = pac::TIMER0;

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
