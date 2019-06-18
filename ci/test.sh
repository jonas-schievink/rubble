#!/bin/bash

set -o errexit

TARGET_BUILD=${TARGET_BUILD:-thumbv7em-none-eabi}

# First, check formatting.
cargo fmt --all -- --check

# Run all tests in the workspace
cargo test --all

# Check that the device crates build with all feature combinations.
# Only use `cargo check` because the PAC crates are very slow to build.
(
    cd rubble-nrf52
    cargo check --features="52810"
    cargo check --features="52832"
    cargo check --features="52840"
)

# Check that the demo app builds with all feature combination.
# Here we do a proper build to also make sure linking the final binary works.
(
    cd rubble-demo
    cargo build --target $TARGET_BUILD --no-default-features
    cargo build --target $TARGET_BUILD
)
