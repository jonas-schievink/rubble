//! Example user configuration for the demo.
//!
//! Put your actual configuration in `config.rs` (which should automatically start out as a copy of
//! this `config.example.rs` file).

config! {
    // The baudrate to configure the UART with.
    // Any variant of `nrf52810_hal::uarte::Baudrate` is accepted.
    baudrate = BAUD115200;

    // UART TX and RX pins.
    // Must be field names of the `nrf52810_hal::gpio::p0::Parts` struct.
    tx_pin = p0_20;
    rx_pin = p0_19;
}
