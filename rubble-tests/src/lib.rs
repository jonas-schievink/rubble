//! Additional tests for Rubble, integrated into the `cargo test` workflow.
//!
//! This is basically a free-form script the will invoke Cargo to build stuff. It's wrapped in a
//! unit test to integrate it into `cargo test`, which should give a Just Works™ experience.

#![cfg(test)]

use glob::glob;
use std::process::Command;
use std::{env, fs};

fn cargo(args: impl AsRef<str>, dir: &str) {
    let cargo = env::var("CARGO").unwrap();
    let status = Command::new(cargo)
        .current_dir(dir)
        .args(args.as_ref().split_whitespace()) // FIXME split properly
        .status()
        .unwrap();
    assert!(status.success(), "{}", status);
}

#[test]
fn additional_tests() {
    // Tests execute in the dir containing the packages `Cargo.toml`. Go to the
    // Rubble root dir. Note that this affects the whole test executable.
    env::set_current_dir("..").unwrap();

    // Enables ring and runs the ECDH test suite on it.
    cargo("test -p rubble --features ring -- ecdh", "rubble");

    // Checks that rubble-nrf5x builds on all supported architectures.
    let targets = [
        (
            "thumbv7em-none-eabi",
            &["52840", "52833", "52832", "52811", "52810", "52805"][..],
        ),
        ("thumbv6m-none-eabi", &["51"][..]),
    ];
    for (target, features) in &targets {
        for feature in *features {
            cargo(
                format!("check --features {} --target {}", feature, target),
                "rubble-nrf5x",
            );
        }
    }

    // Checks that the demos build correctly.
    let features = ["52840", "52833", "52832", "52811", "52810", "52805"];
    let target = "thumbv7em-none-eabi";
    for demo in glob("demos/nrf52*").unwrap() {
        let demo = demo.unwrap().display().to_string();
        for feature in &features {
            cargo(
                format!("build --target {} --features {}", target, feature),
                &demo,
            );
            cargo(
                format!(
                    "build --target {} --features {} --no-default-features",
                    target, feature
                ),
                &demo,
            );
        }
    }

    // Check formatting. Needs to be done last because some demos copy files around.
    cargo("fmt --all -- --check", ".");

    // Generate documentation as part of the test suite. This ensures they always build.
    cargo("doc --no-deps -p rubble -p rubble-nrf5x", "rubble-docs");
    fs::write(
        "target/doc/index.html",
        "<meta http-equiv=refresh content=0;url=rubble/index.html>",
    )
    .unwrap();
}
