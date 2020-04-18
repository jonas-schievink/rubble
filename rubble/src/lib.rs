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

// We're `#[no_std]`, except when we're testing
#![cfg_attr(not(test), no_std)]
// Deny a few warnings in doctests, since rustdoc `allow`s many warnings by default
#![doc(test(attr(deny(unused_imports, unused_must_use))))]
#![warn(rust_2018_idioms)]
// The claims of this lint are dubious, disable it
#![allow(clippy::trivially_copy_pass_by_ref)]

#[macro_use]
mod log;
#[macro_use]
mod utils;
pub mod att;
pub mod beacon;
pub mod bytes;
pub mod config;
mod error;
pub mod gatt;
pub mod l2cap;
pub mod link;
pub mod phy;
pub mod security;
pub mod time;
pub mod uuid;

pub use self::error::Error;

use self::link::llcp::VersionNumber;

/// Version of the Bluetooth specification implemented by Rubble.
pub const BLUETOOTH_VERSION: VersionNumber = VersionNumber::V4_2;
