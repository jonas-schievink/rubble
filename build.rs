use std::fs;

fn main() {
    // Create default .gdbinit if none exists
    if fs::metadata(".gdbinit").is_err() {
        fs::copy(".gdbinit-openocd", ".gdbinit").unwrap();
    }

    println!("cargo:rerun-if-changed=build.rs");
}
