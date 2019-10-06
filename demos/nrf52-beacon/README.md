# `nrf52-beacon`

This demo is a very simple BLE beacon that is broadcast 3 times per second.

It does not enable logging and is kept simple and minimal.

The demo works with the nRF52810, nRF52832, and nRF52840. To run it, one of the
target devices has to be enabled via a Cargo feature:

    cargo run --feature 52810
