#!/bin/bash

set -o errexit

RUSTFLAGS=${RUSTFLAGS:---deny warnings}

# First, check formatting.
echo "Checking code formatting..."
cargo fmt --all -- --check

# Run all tests in the workspace
echo "Running tests with Cargo..."
cargo test --all

# Check that the device crates build with all feature combinations.
# Only use `cargo check` because the PAC crates are very slow to build.
(
    TARGET=thumbv7em-none-eabi
    echo "Checking rubble-nrf52 for $TARGET..."
    cd rubble-nrf52
    cargo check --features="52810" --target "$TARGET"
    cargo check --features="52832" --target "$TARGET"
    cargo check --features="52840" --target "$TARGET"
)

# Check that the demo app builds with all feature combinations.
# Here we do a proper build to also make sure linking the final binary works.
for dir in demos/nrf52*; do
    (
        TARGET=thumbv7em-none-eabi
        echo "Building $dir for $TARGET..."
        cd "$dir"
        cargo build --target "$TARGET" --no-default-features
        cargo build --target "$TARGET"
    )
done

# Check that the core library builds on thumbv6
echo "Building rubble for thumbv6m-none-eabi..."
cargo check -p rubble --target thumbv6m-none-eabi
