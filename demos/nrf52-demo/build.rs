use std::{fs, path::Path};

fn main() {
    if !Path::new("src/config.rs").exists() {
        fs::copy("src/config.example.rs", "src/config.rs").unwrap();
    }
}
