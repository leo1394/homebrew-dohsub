use std::{env, fs};

fn main() {
    println!("cargo:rerun-if-changed=VERSION.txt");
    println!("cargo:rerun-if-changed=Cargo.toml");
    let version=fs::read_to_string("VERSION.txt").expect("Cannot read VERSION.txt");
    assert_eq!(version.trim(),env::var("CARGO_PKG_VERSION").unwrap(),"Cargo.toml version must match VERSION.txt");
}
