//! A small BLE stack.
//!
//! Tries to adhere to the *Bluetooth Core Specification v4.2*. Versions below that will not be
//! supported due to security issues.

#[macro_use]
mod utils;
pub mod att;
pub mod beacon;
mod bytes;
mod crc;
mod error;
pub mod l2cap;
pub mod link;
pub mod phy;
pub mod security_manager;
pub mod time;
pub mod uuid;

pub use self::error::Error;

use self::link::data::VersionNumber;

/// Version of the Bluetooth specification implemented by rubble.
pub const BLUETOOTH_VERSION: VersionNumber = VersionNumber::V4_2;
