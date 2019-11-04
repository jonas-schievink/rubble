#!/usr/bin/env bash

set -o errexit

RUSTFLAGS=${RUSTFLAGS:---deny warnings}

# Run unit tests. We'd prefer to run `cargo test --all`, but some packages
# require enabling Cargo features, which Cargo does not support in that case.
echo "Running tests with Cargo..."
cargo test -p rubble

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

# Check that the demo apps build with all supported feature combinations.
# Here we do a proper build to also make sure linking the final binary works.
for demo in demos/nrf52*; do
    for device in 52810 52832 52840; do
        (
            TARGET=thumbv7em-none-eabi
            echo "Building $demo for device $device, target $TARGET..."
            cd "$demo"
            cargo build --target "$TARGET" --features "$device"
            cargo build --target "$TARGET" --features "$device" --no-default-features
        )
    done
done

# Check that the core library builds on thumbv6
echo "Building rubble for thumbv6m-none-eabi..."
cargo check -p rubble --target thumbv6m-none-eabi

# Lastly, check formatting. We'd like to do this earlier, but some crates copy
# module files around in their build scripts, so they can only be formatted once
# they've been built at least once.
echo "Checking code formatting..."
cargo fmt --all -- --check

# Build documentation.
(
    echo "Generating documentation..."
    cd rubble-docs
    cargo doc --no-deps -p rubble -p rubble-nrf52
)
