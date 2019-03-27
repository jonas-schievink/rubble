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

// trace and debug are exactly the same right now

macro_rules! trace {
    // Special-case when not using any formatting (this is *much* faster than going through
    // `core::fmt`)
    ($logger:expr, $s:literal) => {{
        #[allow(unused_imports)]
        use core::fmt::Write as _;
        $logger.write_str(concat!($s, "\n")).unwrap();
    }};
    ($logger:expr, $($t:tt)+) => {{
        #[allow(unused_imports)]
        use core::fmt::Write as _;
        writeln!($logger, $($t)+).unwrap();
    }};
}

macro_rules! debug {
    ($logger:expr, $($t:tt)+) => {{
        trace!($logger, $($t)+);
    }};
}
