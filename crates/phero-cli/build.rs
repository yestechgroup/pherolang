//! Forward the target triple the binary is compiled for.
//!
//! The release matrix ships one `pherolang` binary per supported target
//! (epic #270, R3-09; Linux only per ADR 0004), and `--version` has to say
//! *which* one a user runs — otherwise a bug report from an aarch64-linux box
//! is indistinguishable from an x86_64-linux one. Cargo sets `TARGET` for
//! every build script; this forwards it as a compile-time environment variable
//! that `main.rs` folds into the clap `long_version`.

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!(
        "cargo:rustc-env=PHERO_BUILD_TARGET={}",
        std::env::var("TARGET").unwrap()
    );
}
