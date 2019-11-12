//! Logging-related utilities and adapters.

use {
    bbqueue::{GrantW, Producer},
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
            data_lost: false,
        }
    }
}

impl fmt::Write for BbqLogger {
    fn write_str(&mut self, msg: &str) -> fmt::Result {
        let mut msg_bytes = msg.as_bytes();
        while !msg_bytes.is_empty() {
            let data_lost_msg_bytes_len = if self.data_lost {
                DATA_LOST_MSG.as_bytes().len()
            } else {
                0
            };
            let total_bytes = data_lost_msg_bytes_len + msg_bytes.len();

            match self.p.grant_max(total_bytes) {
                Ok(grant) => {
                    let mut granted_buf = GrantedBuffer::new(grant);
                    if self.data_lost {
                        granted_buf.append(DATA_LOST_MSG.as_bytes());
                        self.data_lost = false;
                    }
                    let appended_len = granted_buf.append(msg_bytes);
                    msg_bytes = &msg_bytes[appended_len..];
                    granted_buf.commit(&mut self.p);
                }
                Err(_) => {
                    self.data_lost = true;
                    break;
                }
            };
        }

        Ok(())
    }
}

/// Wraps a granted buffer and provides convenience methods to append data and commit
struct GrantedBuffer {
    grant: GrantW,
    written: usize,
}

impl GrantedBuffer {
    pub fn new(grant: GrantW) -> Self {
        GrantedBuffer { grant, written: 0 }
    }

    fn append(&mut self, data: &[u8]) -> usize {
        let buffer = self.grant.buf();
        let remaining = buffer.len() - self.written;
        let written = usize::min(remaining, data.len());
        let write_range = self.written..self.written + written;
        buffer[write_range].copy_from_slice(&data[..written]);
        self.written += written;
        written
    }

    pub fn commit(mut self, producer: &mut Producer) {
        let buffer = self.grant.buf();
        producer.commit(buffer.len(), self.grant)
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
