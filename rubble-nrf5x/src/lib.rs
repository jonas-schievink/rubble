//! A Rubble BLE driver for the nRF51/nRF52-series radios.

#![no_std]
#![warn(rust_2018_idioms)]

#[cfg(feature = "51")]
use nrf51_pac as pac;

#[cfg(feature = "52810")]
use nrf52810_pac as pac;

#[cfg(feature = "52811")]
use nrf52811_pac as pac;

#[cfg(feature = "52832")]
use nrf52832_pac as pac;

#[cfg(feature = "52833")]
use nrf52833_pac as pac;

#[cfg(feature = "52840")]
use nrf52840_pac as pac;

pub mod radio;
pub mod timer;
pub mod utils;
