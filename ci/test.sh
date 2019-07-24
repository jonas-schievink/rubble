#!/bin/bash

set -o errexit

TARGET_BUILD=${TARGET_BUILD:-thumbv7em-none-eabi}

# First, check formatting.
echo "Checking code formatting..."
cargo fmt --all -- --check

# Run all tests in the workspace
echo "Running tests with Cargo..."
cargo test --all

# Check that the device crates build with all feature combinations.
# Only use `cargo check` because the PAC crates are very slow to build.
(
    echo "Checking rubble-nrf52..."
    cd rubble-nrf52
    cargo check --features="52810"
    cargo check --features="52832"
    cargo check --features="52840"
)

# Check that the demo app builds with all feature combinations.
# Here we do a proper build to also make sure linking the final binary works.
for dir in demos/*; do
    (
        echo "Checking $dir..."
        cd "$dir"
        cargo build --target "$TARGET_BUILD" --no-default-features
        cargo build --target "$TARGET_BUILD"
    )
done
