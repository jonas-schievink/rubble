//! A small BLE stack.
//!
//! Tries to adhere to the *Bluetooth Core Specification v4.2*. Versions below that will not be
//! supported due to security issues.

#[macro_use]
mod utils;
pub mod beacon;
mod bytes;
mod crc;
mod error;
pub mod l2cap;
pub mod link;
pub mod phy;
mod responder;
pub mod time;
pub mod uuid;

pub use self::error::Error;
pub use self::responder::Responder;
