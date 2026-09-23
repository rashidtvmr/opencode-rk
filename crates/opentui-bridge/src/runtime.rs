#![forbid(unsafe_code)]

//! Runtime platform detection and native asset path resolution.
//!
//! Mirrors the OpenTUI (opentui @4954312d) TypeScript runtime detection:
//! - `packages/core/src/node-asset-target.ts` defines `NodeAssetTarget` and
//!   `getNativeAssetDescriptor` which maps `(platform, arch, libc)` to a
//!   native library file name and package-scoped asset key.
//! - `packages/core/src/platform/runtime-assets.{bun,node}.ts` and
//!   `assets.ts` resolve the native library path using `OTUI_ASSET_ROOT`
//!   or a platform-specific dynamic import.

// ---------------------------------------------------------------------------
// Public API markers: Runtime, detect, AssetPath, resolve
// ---------------------------------------------------------------------------

/// Detected operating system family, mapping `process.platform` in Node.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Os {
    Darwin,
    Linux,
    Windows,
}

/// CPU architecture, mapping `process.arch` in Node.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Arch {
    X64,
    Arm64,
}

/// Linux libc variant, mapping `OPENTUI_LIBC` env var in Node.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Libc {
    Glibc,
    Musl,
}

/// Detected runtime environment for native asset resolution.
///
/// Mirrors `NodeAssetTarget` in `packages/core/src/node-asset-target.ts`.
/// Each variant carries the CPU arch and optional libc for that OS, matching
/// the platform/arch/libc triple OpenTUI uses to select the correct
/// `@opentui/core-{platform}-{arch}{libc}` native package and its library
/// file name.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Runtime {
    /// macOS (`process.platform === "darwin"`).
    Darwin { arch: Arch, libc: Option<Libc> },
    /// Linux (`process.platform === "linux"`). libc is glibc or musl.
    Linux { arch: Arch, libc: Option<Libc> },
    /// Windows (`process.platform === "win32"`).
    Windows { arch: Arch },
}

/// Result of `detect`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuntimeDetectError {
    /// The host OS is not one of darwin/linux/win32.
    UnsupportedOs(String),
    /// The host CPU arch is not x64 or arm64.
    UnsupportedArch(String),
    /// `OPENTUI_LIBC` was set to something other than glibc/musl on Linux.
    UnsupportedLibc(String),
    /// `OPENTUI_LIBC` was set on a non-Linux platform.
    LibcOnlyOnLinux,
}

impl Runtime {
    /// Operating-system family for this runtime.
    pub const fn os(&self) -> Os {
        match self {
            Runtime::Darwin { .. } => Os::Darwin,
            Runtime::Linux { .. } => Os::Linux,
            Runtime::Windows { .. } => Os::Windows,
        }
    }

    /// CPU architecture for this runtime.
    pub const fn arch(&self) -> Arch {
        match self {
            Runtime::Darwin { arch, .. }
            | Runtime::Linux { arch, .. }
            | Runtime::Windows { arch } => *arch,
        }
    }

    /// Linux libc variant, if set.
    pub const fn libc(&self) -> Option<Libc> {
        match self {
            Runtime::Darwin { libc, .. } => *libc,
            Runtime::Linux { libc, .. } => *libc,
            Runtime::Windows { .. } => None,
        }
    }

    /// Native library file name for this target (mirrors `NATIVE_FILE_NAMES`).
    ///
    /// - darwin -> `libopentui.dylib`
    /// - linux  -> `libopentui.so`
    /// - win32  -> `opentui.dll`
    pub const fn library_file_name(&self) -> &'static str {
        match self.os() {
            Os::Darwin => "libopentui.dylib",
            Os::Linux => "libopentui.so",
            Os::Windows => "opentui.dll",
        }
    }

    /// libc suffix appended to the package name, only `-musl` on Linux+ musl.
    fn libc_suffix(&self) -> &'static str {
        match *self {
            Runtime::Linux {
                libc: Some(Libc::Musl),
                ..
            } => "-musl",
            _ => "",
        }
    }

    /// Package name `@opentui/core-{platform}-{arch}{libcSuffix}`.
    pub fn package_name(&self) -> String {
        let platform = match self.os() {
            Os::Darwin => "darwin",
            Os::Linux => "linux",
            Os::Windows => "win32",
        };
        let arch = match self.arch() {
            Arch::X64 => "x64",
            Arch::Arm64 => "arm64",
        };
        format!("@opentui/core-{}-{}{}", platform, arch, self.libc_suffix())
    }
}

/// Detect the current host runtime using compile-time `cfg` flags, reading
/// `OPENTUI_LIBC` from the environment for Linux libc selection.
///
/// This mirrors `getCurrentNodeAssetTarget` in
/// `packages/core/src/node-asset-target.ts`.
pub fn detect() -> Result<Runtime, RuntimeDetectError> {
    let os = if cfg!(target_os = "macos") {
        Os::Darwin
    } else if cfg!(target_os = "linux") {
        Os::Linux
    } else if cfg!(target_os = "windows") {
        Os::Windows
    } else {
        return Err(RuntimeDetectError::UnsupportedOs(cfg!(target_os).to_string()));
    };

    let arch = if cfg!(target_arch = "x86_64") {
        Arch::X64
    } else if cfg!(target_arch = "aarch64") {
        Arch::Arm64
    } else {
        return Err(RuntimeDetectError::UnsupportedArch(cfg!(target_arch).to_string()));
    };

    match os {
        Os::Linux => {
            let libc = match std::env::var("OPENTUI_LIBC") {
                Ok(val) if val.is_empty() || val == "glibc" => Some(Libc::Glibc),
                Ok(val) if val == "musl" => Some(Libc::Musl),
                Ok(val) => return Err(RuntimeDetectError::UnsupportedLibc(val)),
                Err(_) => None,
            };
            Ok(Runtime::Linux { arch, libc })
        }
        Os::Darwin => {
            // libc is only valid on Linux.
            if std::env::var("OPENTUI_LIBC").map(|v| !v.is_empty()).unwrap_or(false) {
                return Err(RuntimeDetectError::LibcOnlyOnLinux);
            }
            Ok(Runtime::Darwin { arch, libc: None })
        }
        Os::Windows => {
            if std::env::var("OPENTUI_LIBC").map(|v| !v.is_empty()).unwrap_or(false) {
                return Err(RuntimeDetectError::LibcOnlyOnLinux);
            }
            Ok(Runtime::Windows { arch })
        }
    }
}

/// Asset path descriptor holding the OpenTUI asset key.
///
/// The key is of the form `@opentui/core-{platform}-{arch}{libc}/{fileName}`
/// and is used to look up a file under `OTUI_ASSET_ROOT` (mirrors
/// `resolveAssetRootPath` in `assets.ts`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssetPath {
    pub key: String,
    pub file_name: String,
}

impl AssetPath {
    /// Build an `AssetPath` for the given runtime target.
    ///
    /// key = `{packageName}/{libraryFileName}`
    pub fn for_runtime(rt: &Runtime) -> Self {
        let file_name = rt.library_file_name().to_string();
        let key = format!("{}/{}", rt.package_name(), file_name);
        AssetPath { key, file_name }
    }

    /// Resolve to a concrete filesystem path.
    ///
    /// Mirrors `resolveAssetRootPath` in `packages/core/src/platform/assets.ts`:
    /// if `OTUI_ASSET_ROOT` is set and absolute, join it with `self.key` and
    /// verify the result is a file. Otherwise return the key itself, which
    /// serves as the package specifier the caller may resolve through their
    /// own resolution mechanism.
    pub fn resolve(&self) -> Result<String, String> {
        let root = std::env::var("OTUI_ASSET_ROOT").ok();
        if let Some(root) = root {
            if root.is_empty() {
                return Ok(self.key.clone());
            }
            if !std::path::Path::new(&root).is_absolute() {
                return Err(format!(
                    "OTUI_ASSET_ROOT must be an absolute directory, got {:?}",
                    root
                ));
            }
            let path = std::path::Path::new(&root).join(&self.key);
            if !path.is_file() {
                return Err(format!(
                    "Missing OpenTUI asset {:?} at {:?}",
                    self.key,
                    path.display()
                ));
            }
            return Ok(path.to_string_lossy().into_owned());
        }
        Ok(self.key.clone())
    }
}

// ---------------------------------------------------------------------------
// Tests (RED phase — written before implementation was finalized)
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detect_matches_host_triple() {
        // detect() must succeed on the host; it validates invariants internally.
        let rt = detect().expect("detect should succeed on this host");
        // libc may only be present on Linux.
        if rt.os() != Os::Linux {
            assert_eq!(rt.libc(), None);
        }
    }

    #[test]
    fn darwin_arm64_library_name() {
        let rt = Runtime::Darwin {
            arch: Arch::Arm64,
            libc: None,
        };
        assert_eq!(rt.library_file_name(), "libopentui.dylib");
    }

    #[test]
    fn linux_x64_glibc_package_name() {
        let rt = Runtime::Linux {
            arch: Arch::X64,
            libc: Some(Libc::Glibc),
        };
        assert_eq!(rt.package_name(), "@opentui/core-linux-x64");
    }

    #[test]
    fn linux_arm64_musl_package_name_has_libc_suffix() {
        let rt = Runtime::Linux {
            arch: Arch::Arm64,
            libc: Some(Libc::Musl),
        };
        assert_eq!(rt.package_name(), "@opentui/core-linux-arm64-musl");
    }

    #[test]
    fn windows_x64_library_file_name() {
        let rt = Runtime::Windows { arch: Arch::X64 };
        assert_eq!(rt.library_file_name(), "opentui.dll");
    }

    #[test]
    fn asset_path_key_for_linux_musl() {
        let rt = Runtime::Linux {
            arch: Arch::Arm64,
            libc: Some(Libc::Musl),
        };
        let ap = AssetPath::for_runtime(&rt);
        assert_eq!(ap.file_name, "libopentui.so");
        assert_eq!(ap.key, "@opentui/core-linux-arm64-musl/libopentui.so");
    }

    #[test]
    fn resolve_without_env_returns_key() {
        // Ensure OTUI_ASSET_ROOT is unset for this test.
        std::env::remove_var("OTUI_ASSET_ROOT");
        let ap = AssetPath {
            key: "@opentui/core-darwin-arm64/libopentui.dylib".to_string(),
            file_name: "libopentui.dylib".to_string(),
        };
        assert_eq!(
            ap.resolve().unwrap(),
            "@opentui/core-darwin-arm64/libopentui.dylib"
        );
    }

    #[test]
    fn resolve_with_absolute_root_joins_key() {
        let tmp = std::env::temp_dir();
        let root = tmp.canonicalize().unwrap();
        std::env::set_var("OTUI_ASSET_ROOT", root.to_string_lossy().to_string());
        let ap = AssetPath {
            key: "@opentui/core-darwin-arm64/libopentui.dylib".to_string(),
            file_name: "libopentui.dylib".to_string(),
        };
        // File does not exist -> should error (mirrors stat check in assets.ts).
        let err = ap.resolve().unwrap_err();
        assert!(err.contains("Missing OpenTUI asset"), "got: {err}");
        std::env::remove_var("OTUI_ASSET_ROOT");
    }

    #[test]
    fn resolve_non_absolute_root_errors() {
        std::env::set_var("OTUI_ASSET_ROOT", "relative/path");
        let ap = AssetPath {
            key: "some/asset".to_string(),
            file_name: "a.so".to_string(),
        };
        let err = ap.resolve().unwrap_err();
        assert!(
            err.contains("OTUI_ASSET_ROOT must be an absolute directory"),
            "got: {err}"
        );
        std::env::remove_var("OTUI_ASSET_ROOT");
    }
}
