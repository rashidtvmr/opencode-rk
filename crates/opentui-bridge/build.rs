//! Build script: wire native libopentui when `native` feature enabled.
fn main() {
    if std::env::var("CARGO_FEATURE_NATIVE").is_ok() {
        let triple = std::env::var("TARGET").unwrap_or_default();
        let manifest = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let libdir = manifest.join("native").join("lib").join(&triple);
        println!("cargo:rustc-link-search=native={}", libdir.display());
        let a = libdir.join("libopentui.a");
        let so = libdir.join("libopentui.so");
        if a.exists() {
            println!("cargo:rustc-link-lib=static=opentui");
        } else {
            println!("cargo:rustc-link-lib=dylib=opentui");
        }
        println!("cargo:rerun-if-changed={}", libdir.display());
        if !a.exists() && !so.exists() {
            panic!(
                "native libopentui artifact missing for triple '{triple}'.\n\
                 expected: {} or {}\n\
                 build it from /home/rashid/projects/opentui branch rust-bridge first.",
                a.display(),
                so.display()
            );
        }
    } else {
        // Artifact not built yet: warn only, no nm gate.
        println!("cargo:warning=opentui-bridge: native feature off; skipping link gate");
    }
}
