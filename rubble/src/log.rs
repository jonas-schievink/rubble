#[cfg(feature = "log")]
macro_rules! error {
    ($($t:tt)*) => {{ log::error!($($t)*); }};
}

#[cfg(feature = "log")]
macro_rules! warn {
    ($($t:tt)*) => {{ log::warn!($($t)*); }};
}

#[cfg(feature = "log")]
macro_rules! info {
    ($($t:tt)*) => {{ log::info!($($t)*); }};
}

#[cfg(feature = "log")]
macro_rules! debug {
    ($($t:tt)*) => {{ log::debug!($($t)*); }};
}

#[cfg(feature = "log")]
macro_rules! trace {
    ($($t:tt)*) => {{ log::trace!($($t)*); }};
}

#[cfg(not(feature = "log"))]
macro_rules! error {
    ($($t:tt)*) => {{ format_args!($($t)*); }};
}

#[cfg(not(feature = "log"))]
macro_rules! warn {
    ($($t:tt)*) => {{ format_args!($($t)*); }};
}

#[cfg(not(feature = "log"))]
macro_rules! info {
    ($($t:tt)*) => {{ format_args!($($t)*); }};
}

#[cfg(not(feature = "log"))]
macro_rules! debug {
    ($($t:tt)*) => {{ format_args!($($t)*); }};
}

#[cfg(not(feature = "log"))]
macro_rules! trace {
    ($($t:tt)*) => {{ format_args!($($t)*); }};
}
