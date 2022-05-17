**Note**: this project is not currently maintained.

# Rubble

[![crates.io](https://img.shields.io/crates/v/rubble.svg)](https://crates.io/crates/rubble)
[![docs.rs](https://docs.rs/rubble/badge.svg)](https://docs.rs/rubble/)
![CI](https://github.com/jonas-schievink/rubble/workflows/CI/badge.svg)

Rubble is a Bluetooth® Low Energy compatible protocol stack for embedded Rust.

Currently, Rubble supports Nordic's nRF52-series of MCUs. However, it was
designed to be hardware-independent, so support crates for other MCU families
are always welcome.

[Internal API documentation (master)][docs-master]

**NOTE: None of this has passed the Bluetooth® Qualification Process, so it
must not be used in commercial products!**

## Usage

See [demos](./demos/) for a few self-contained usage examples.

API documentation can be viewed [on docs.rs][docs-rs] for the latest crates.io release,
or [here for API docs generated from master][docs-master].

[docs-rs]: https://docs.rs/rubble/
[docs-master]: https://jonas-schievink.github.io/rubble/

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md).

## License

0-Clause BSD License ([LICENSE](LICENSE)).
