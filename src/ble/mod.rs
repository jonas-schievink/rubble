//! A small Bluetooth Low Energy stack.
//!
//! Tries to adhere to the Bluetooth Core Specification v4.1 (for now).

#[macro_use]
mod utils;
mod crc;
pub mod link;
pub mod phy;

pub enum Error {
    /// Packet specified an invalid length value or was too short.
    InvalidLength,

    #[doc(hidden)]
    __Nonexhaustive,
}
