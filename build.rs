use std::{
    env,
    fs::{self, File},
    io::Write,
    path::PathBuf,
};

fn main() {
    // Put the linker script somewhere the linker can find it
    let out = &PathBuf::from(env::var_os("OUT_DIR").unwrap());
    File::create(out.join("memory.x"))
        .unwrap()
        .write_all(include_bytes!("memory.x"))
        .unwrap();
    println!("cargo:rustc-link-search={}", out.display());

    // Create default .gdbinit if none exists
    if fs::metadata(".gdbinit").is_err() {
        fs::copy(".gdbinit-openocd", ".gdbinit").unwrap();
    }

    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=memory.x");
}
