use core::fmt::{self, Write};

/// Trait for embedded loggers.
pub trait Logger: Write {}

impl<T: Write> Logger for T {}

/// A `Logger` that disposes all messages.
pub struct NoopLogger;

impl Write for NoopLogger {
    fn write_str(&mut self, _: &str) -> fmt::Result {
        Ok(())
    }
}

macro_rules! trace {
    ($logger:expr, $($t:tt)+) => {{
        writeln!($logger, $($t)+).unwrap();
    }};
}
