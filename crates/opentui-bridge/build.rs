//! Build script: wire the platform-native libopentui when the `native`
//! feature is enabled. Artifacts are vendored per Rust target triple under
//! `native/lib/<triple>`; development and release builds use the same gate.

use std::path::{Path, PathBuf};

fn exists(dir: &Path, name: &str) -> bool {
    dir.join(name).is_file()
}

fn main() {
    if std::env::var("CARGO_FEATURE_NATIVE").is_err() {
        println!("cargo:warning=opentui-bridge: native feature off; skipping link gate");
        return;
    }

    let triple = std::env::var("TARGET").unwrap_or_default();
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let libdir = manifest.join("native").join("lib").join(&triple);
    println!("cargo:rustc-link-search=native={}", libdir.display());
    println!("cargo:rerun-if-changed={}", libdir.display());

    let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    let target_env = std::env::var("CARGO_CFG_TARGET_ENV").unwrap_or_default();

    let static_unix = exists(&libdir, "libopentui.a");
    let linux_shared = exists(&libdir, "libopentui.so");
    let mac_shared = exists(&libdir, "libopentui.dylib");
    let windows_import = exists(&libdir, "opentui.lib") || exists(&libdir, "libopentui.dll.a");
    let windows_runtime = exists(&libdir, "opentui.dll");

    match target_os.as_str() {
        "macos" if static_unix => {
            println!("cargo:rustc-link-lib=static=opentui");
        }
        "macos" if mac_shared => {
            println!("cargo:rustc-link-lib=dylib=opentui");
        }
        "linux" if static_unix => {
            println!("cargo:rustc-link-lib=static=opentui");
        }
        "linux" if linux_shared => {
            println!("cargo:rustc-link-lib=dylib=opentui");
        }
        "windows" if windows_import && windows_runtime => {
            // MSVC consumes opentui.lib; GNU targets consume libopentui.dll.a.
            // The DLL is still required beside the installed executable.
            println!("cargo:rustc-link-lib=dylib=opentui");
        }
        _ => {
            let expected = match target_os.as_str() {
                "macos" => "libopentui.a or libopentui.dylib",
                "linux" => "libopentui.a or libopentui.so",
                "windows" if target_env == "msvc" => "opentui.lib and opentui.dll",
                "windows" => "libopentui.dll.a and opentui.dll",
                _ => "a supported libopentui static/shared artifact",
            };
            panic!(
                "native libopentui artifact missing for target '{triple}'.\n\
                 expected {expected} in {}.\n\
                 Build the pinned OpenTUI native library for this target and vendor it under native/lib/{triple}.",
                libdir.display()
            );
        }
    }
}
