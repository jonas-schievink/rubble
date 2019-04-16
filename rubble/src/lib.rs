//! An experimental BLE stack.
//!
//! Tries to adhere to the *Bluetooth Core Specification v4.2* (at least for now).
//!
//! # Using the stack
//!
//! Rubble is runtime and hardware-agnostic: It does not need an RTOS (although you can certainly
//! use one if you want) and provides hardware interfaces that need to be implemented once for
//! every supported MCU family.
//!
//! The only part that interacts directly with platform-specific interfaces is [`link`], Rubble's
//! BLE Link-Layer implementation. You have to provide it with a few hardware-specific services:
//! * A microsecond-precision [`Timer`].
//! * A [`Transmitter`] that can send data and advertising channel packets.
//! * A processor for `link::Cmd`, which tells the support code when to call Rubble's functions
//!   again.
//!
//! [`link`]: link/index.html
//! [`Timer`]: time/trait.Timer.html
//! [`Transmitter`]: link/trait.Transmitter.html

#![no_std]

#[macro_use]
mod utils;
pub mod att;
pub mod beacon;
pub mod bytes;
mod crc;
mod error;
pub mod gatt;
pub mod l2cap;
pub mod link;
pub mod phy;
pub mod security_manager;
pub mod time;
pub mod uuid;

pub use self::error::Error;

use self::link::data::VersionNumber;

/// Version of the Bluetooth specification implemented by Rubble.
pub const BLUETOOTH_VERSION: VersionNumber = VersionNumber::V4_2;
