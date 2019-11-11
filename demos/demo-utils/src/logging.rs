//! Logging-related utilities and adapters.

use {
    bbqueue::Producer,
    core::{cell::RefCell, fmt},
    cortex_m::interrupt::{self, Mutex},
    log::{Log, Metadata, Record},
    rubble::time::Timer,
};


const DATA_LOST_MSG: &str = "…\n";

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
    data_lost: bool,
}

impl BbqLogger {
    pub fn new(p: Producer) -> Self {
        Self {
            p,
            data_lost: false
        }
    }
}

impl fmt::Write for BbqLogger {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        let data_lost_msg_bytes: usize = if self.data_lost {
            DATA_LOST_MSG.as_bytes().len()
        }
        else {
            0
        };

        let mut bytes = s.as_bytes();

        while !bytes.is_empty() {
            match self.p.grant_max(bytes.len() + data_lost_msg_bytes) {
                Ok(mut grant) => {
                    let granted_buf = grant.buf();
                    if self.data_lost {
                        granted_buf[..data_lost_msg_bytes].copy_from_slice(DATA_LOST_MSG.as_bytes());
                        self.data_lost = false;
                    }
                    let size = granted_buf.len() - data_lost_msg_bytes;
                    granted_buf[data_lost_msg_bytes..].copy_from_slice(&bytes[..size]);
                    bytes = &bytes[size..];
                    self.p.commit(granted_buf.len(), grant);
                },
                Err(_) => {
                    self.data_lost = true;
                    bytes = &[];
                }
            };
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
