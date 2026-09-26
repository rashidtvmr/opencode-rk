#![forbid(unsafe_code)]
//! Installer copy-beside-exe names for the native OpenTUI library.
//! Mirrors native_arch.rs MATRIX shared artifacts; build.rs links the
//! import lib on Windows so the runtime DLL must ship beside the exe.

/// Shared library filename that must sit beside the installed executable.
#[must_use]
pub fn beside_exe_name(triple: &str) -> &'static str {
    if triple.contains("-pc-windows-") {
        "opentui.dll"
    } else if triple.contains("-apple-") {
        "libopentui.dylib"
    } else {
        "libopentui.so"
    }
}

/// Installer instruction: copy the shared library beside the executable.
#[must_use]
pub fn install_note() -> &'static str {
    "Copy the shared library beside the installed executable (opentui.dll on Windows, libopentui.so on Linux, libopentui.dylib on macOS); Windows links the import library at build time and loads the DLL from beside the exe at runtime."
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn windows_needs_dll_beside_exe() {
        assert_eq!(beside_exe_name("x86_64-pc-windows-msvc"), "opentui.dll");
        assert_eq!(beside_exe_name("x86_64-pc-windows-gnu"), "opentui.dll");
    }

    #[test]
    fn unix_names() {
        assert_eq!(beside_exe_name("x86_64-unknown-linux-gnu"), "libopentui.so");
        assert_eq!(beside_exe_name("aarch64-apple-darwin"), "libopentui.dylib");
    }

    #[test]
    fn note_covers_beside_exe() {
        let n = install_note();
        assert!(n.contains("beside the"));
        assert!(n.contains("opentui.dll"));
        assert!(n.contains("libopentui.so"));
        assert!(n.contains("libopentui.dylib"));
    }
}
