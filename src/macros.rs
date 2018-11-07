/// Prints a line to the host's stderr stream, if available.
macro_rules! heprintln {
    ( $($t:tt)* ) => {{
        use core::fmt::Write;
        match ::cortex_m_semihosting::hio::hstderr() {
            Ok(mut s) => {
                writeln!(s, $($t)*).ok();
            }
            Err(()) => {}
        }
    }};
}

/// Prints to the host's stderr stream, if available.
macro_rules! heprint {
    ( $($t:tt)* ) => {{
        use core::fmt::Write;
        match ::cortex_m_semihosting::hio::hstderr() {
            Ok(mut s) => {
                write!(s, $($t)*).ok();
            }
            Err(()) => {}
        }
    }};
}
