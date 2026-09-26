//! Resolve a bundled dynamic libopentui relative to either CLI executable.
//!
//! The bridge crate selects the native artifact, but its rlib build script
//! cannot set a loader path on the final executable. The installer places the
//! binary in `bin/` and the library in the sibling `lib/` directory.

use std::path::PathBuf;

fn main() {
    if std::env::var_os("CARGO_FEATURE_NATIVE").is_none() {
        return;
    }

    let target = std::env::var("TARGET").expect("Cargo supplies TARGET");
    let libdir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../opentui-bridge/native/lib")
        .join(target);
    println!("cargo:rerun-if-changed={}", libdir.display());

    // The bridge prefers a static archive if both forms exist. Static linking
    // needs no runtime lookup; do not add an unnecessary library search path.
    if libdir.join("libopentui.a").is_file() {
        return;
    }

    let rpath = match std::env::var("CARGO_CFG_TARGET_OS").as_deref() {
        Ok("macos") if libdir.join("libopentui.dylib").is_file() => "@loader_path/../lib",
        Ok("linux") if libdir.join("libopentui.so").is_file() => "$ORIGIN/../lib",
        _ => return, // The bridge's artifact gate reports unsupported/missing targets.
    };

    // Both oc2 and the compatibility binary are built from this crate. Cargo
    // passes the argument directly to rustc, so `$ORIGIN` is not shell-expanded.
    println!("cargo:rustc-link-arg-bins=-Wl,-rpath,{rpath}");
}
