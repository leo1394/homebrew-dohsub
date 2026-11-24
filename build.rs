use std::{env, fs, path::PathBuf, process::Command};

fn main() {
    println!("cargo:rerun-if-changed=VERSION.txt");
    println!("cargo:rerun-if-changed=Cargo.toml");
    println!("cargo:rerun-if-changed=relay-engine");
    println!("cargo:rerun-if-env-changed=GO");
    let target=env::var("TARGET").unwrap();
    let arch=if target.starts_with("aarch64-") {"arm64"} else if target.starts_with("x86_64-") {"amd64"} else {panic!("Unsupported relay architecture")};
    let os=if target.contains("apple-darwin") {"darwin"} else if target.contains("linux") {"linux"} else {panic!("Unsupported relay platform")};
    let output=PathBuf::from(env::var_os("OUT_DIR").unwrap()).join("dohsub-relay");
    let status=Command::new(env::var_os("GO").unwrap_or_else(||"go".into())).current_dir("relay-engine")
        .env("GOOS",os).env("GOARCH",arch).env("CGO_ENABLED","0")
        .args(["build","-trimpath","-ldflags=-s -w","-o"]).arg(&output).arg("./cmd/dohsub-relay").status().expect("Go is required to build the bundled relay engine");
    assert!(status.success(),"Relay engine build failed");
    let version=fs::read_to_string("VERSION.txt").expect("Cannot read VERSION.txt");
    assert_eq!(version.trim(),env::var("CARGO_PKG_VERSION").unwrap(),"Cargo.toml version must match VERSION.txt");
}
