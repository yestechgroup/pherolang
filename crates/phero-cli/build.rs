//! Forward the target triple the binary is compiled for.
//!
//! The release ships one `pherolang` binary for the supported target
//! (epic #270, R3-09; Linux x86_64 only per ADR 0004), and `--version` says
//! *which* one a user runs. Cargo sets `TARGET` for every build script; this
//! forwards it as a compile-time environment variable that `main.rs` folds
//! into the clap `long_version`.

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!(
        "cargo:rustc-env=PHERO_BUILD_TARGET={}",
        std::env::var("TARGET").unwrap()
    );
}
