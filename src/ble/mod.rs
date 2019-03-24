//! A small Bluetooth Low Energy stack.
//!
//! Tries to adhere to the Bluetooth Core Specification v4.2. Versions below
//! that will not be supported due to security issues.

#[macro_use]
mod utils;
#[macro_use]
pub mod log;
pub mod beacon;
mod bytes;
mod crc;
mod error;
pub mod link;
pub mod phy;
pub mod time;
pub mod uuid;

pub use self::error::Error;
