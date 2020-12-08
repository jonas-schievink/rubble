use std::env;

fn main() {
    let features = &["51", "52810", "52832", "52833", "52840"];
    let count = features
        .iter()
        .filter(|name| env::var_os(format!("CARGO_FEATURE_{}", name)).is_some())
        .count();

    match count {
        0 => panic!("no device feature enabled; please enable one of the following features, matching your target device: {:?}", features),
        1 => {},
        _ => panic!("{} device features enabled; please only enable one", count),
    }
}
