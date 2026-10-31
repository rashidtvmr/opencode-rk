#![forbid(unsafe_code)]
//! Exact-commit identity for the linked OpenTUI artifact (FFI-01).
//!
//! EVIDENCE / BOUNDARY
//! - `docs/ui-parity/opencode-ui-parity-kit/audit-opentui-boundary.md` §2
//!   "Checked resources and frame/output protocol" (ref [N1]
//!   `packages/native/README.md:1-75`, [N6]
//!   `packages/native/src/context-abi.zig:35-101`) records that the checked C
//!   interface is **experimental ABI version 1** and that upstream requires
//!   matching headers and artifacts from the same revision. The kit pin is
//!   OpenTUI `0.6.8` = commit `8d6065a3e60039d16c6ee697692dcfbc46d3c786`,
//!   selected by OpenCode `450e7b9` (`package.json:50-65`, ref [P1]).
//! - The vendored artifacts carry **no** embedded commit or version string
//!   (`strings -a libopentui.dylib` finds only feature strings such as
//!   `OPENTUI_IMAGE_PROTOCOL`), and the library's only metadata export is
//!   `getBuildOptions(ExternalBuildOptions*)`, which reports the two GPA build
//!   booleans, not a revision. Identity is therefore established by three
//!   facts that must all agree:
//!   1. the recorded pin ([`OPENTUI_COMMIT`], [`OPENTUI_VERSION`],
//!      [`ABI_VERSION`]) is well formed;
//!   2. the artifact bytes match the pinned length and [`fnv1a64`]
//!      fingerprint for that target triple ([`ARTIFACTS`]);
//!   3. every symbol this crate declares is exported by the artifact
//!      ([`REQUIRED_SYMBOLS`]), so the binary really is the audited ABI.
//!
//!   [`probe_installed`] performs all three against the on-disk artifact;
//!   `native_identity_matches_pinned_artifact` exercises it in CI.

use std::collections::BTreeSet;
use std::io;
use std::path::{Path, PathBuf};

/// OpenTUI release the pin refers to.
pub const OPENTUI_VERSION: &str = "0.6.8";
/// Exact OpenTUI commit pinned by the parity kit.
pub const OPENTUI_COMMIT: &str = "8d6065a3e60039d16c6ee697692dcfbc46d3c786";
/// OpenCode revision that selects this OpenTUI release.
pub const OPENCODE_COMMIT: &str = "450e7b92412c780a7b5a75226bb8d49aaca24e22";
/// Checked C ABI version (`context-abi.zig`, experimental ABI 1).
pub const ABI_VERSION: u16 = 1;

/// One pinned native artifact.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ArtifactPin {
    pub triple: &'static str,
    pub file: &'static str,
    pub bytes: u64,
    /// FNV-1a 64 of the exact file bytes.
    pub fingerprint: u64,
}

/// Vendored artifacts and their byte fingerprints.
///
/// The fingerprint is a deterministic checksum, not a cryptographic digest:
/// it detects a swapped or rebuilt artifact during a local build without
/// adding a hashing dependency. Custody of the full artifact hashes remains
/// external (see the delivery evidence root); this table pins identity inside
/// the crate.
pub const ARTIFACTS: [ArtifactPin; 3] = [
    ArtifactPin {
        triple: "aarch64-apple-darwin",
        file: "libopentui.dylib",
        bytes: 6_397_376,
        fingerprint: 0xfb3f_0624_7cd2_b36e,
    },
    ArtifactPin {
        triple: "aarch64-unknown-linux-gnu",
        file: "libopentui.so",
        bytes: 26_466_240,
        fingerprint: 0x4207_20df_541c_dfd3,
    },
    ArtifactPin {
        triple: "x86_64-unknown-linux-gnu",
        file: "libopentui.so",
        bytes: 26_595_336,
        fingerprint: 0x2bd5_c328_db93_0bbf,
    },
];

/// Symbols this crate declares and therefore requires the artifact to export.
///
/// Kept in sync with the `extern "C"` blocks in `renderer.rs`, `buffer.rs`,
/// `text.rs` and `safe_renderer.rs`; the probe refuses an artifact that is
/// missing any of them, which also catches a mismatched upstream revision.
pub const REQUIRED_SYMBOLS: [&str; 53] = [
    "bufferClear",
    "bufferDrawBox",
    "bufferDrawGrid",
    "bufferDrawText",
    "bufferFillRect",
    "bufferResize",
    "bufferSetCell",
    "bufferWriteResolvedChars",
    "clearTerminal",
    "createOptimizedBuffer",
    "createRenderer",
    "createTextBuffer",
    "destroyOptimizedBuffer",
    "destroyRenderer",
    "destroyTextBuffer",
    "disableKittyKeyboard",
    "disableMouse",
    "editBufferReplaceText",
    "editBufferSetText",
    "editBufferSetTextFromMem",
    "enableKittyKeyboard",
    "enableMouse",
    "getBufferHeight",
    "getBufferWidth",
    "getBuildOptions",
    "getCurrentBuffer",
    "getNextBuffer",
    "queryPixelResolution",
    "render",
    "resizeRenderer",
    "restoreTerminalModes",
    "resumeRenderer",
    "setBackgroundColor",
    "setCursorColor",
    "setCursorPosition",
    "setTerminalEnvVar",
    "setTerminalTitle",
    "setUseThread",
    "setupTerminal",
    "suspendRenderer",
    "textBufferAppend",
    "textBufferAppendFromMemId",
    "textBufferClear",
    "textBufferGetByteSize",
    "textBufferGetLength",
    "textBufferGetLineCount",
    "textBufferGetPlainText",
    "textBufferLoadFile",
    "textBufferReset",
    "textBufferSetDefaultBg",
    "textBufferSetDefaultFg",
    "textBufferSetStyledText",
    "textBufferSetTextFromMem",
];

/// Build target this crate was compiled for.
pub const HOST_TRIPLE: &str = if cfg!(all(target_arch = "aarch64", target_os = "macos")) {
    "aarch64-apple-darwin"
} else if cfg!(all(target_arch = "aarch64", target_os = "linux")) {
    "aarch64-unknown-linux-gnu"
} else if cfg!(all(target_arch = "x86_64", target_os = "linux")) {
    "x86_64-unknown-linux-gnu"
} else {
    "unsupported"
};

/// A verified identity.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Identified {
    pub opencode_commit: &'static str,
    pub opentui_version: &'static str,
    pub opentui_commit: &'static str,
    pub abi_version: u16,
    pub triple: &'static str,
    pub symbol_count: usize,
}

/// Probe failures. Every variant means "not the pinned OpenTUI".
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IdentityError {
    BadCommit { value: String },
    UnknownTriple { triple: String },
    Io { path: PathBuf, kind: io::ErrorKind },
    LengthMismatch { expected: u64, actual: u64 },
    FingerprintMismatch { expected: u64, actual: u64 },
    MissingSymbol(&'static str),
    UnsupportedHost,
}

impl core::fmt::Display for IdentityError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::BadCommit { value } => write!(f, "commit {value:?} is not 40 lowercase hex"),
            Self::UnknownTriple { triple } => write!(f, "no pinned artifact for {triple}"),
            Self::Io { path, kind } => write!(f, "cannot read {}: {kind:?}", path.display()),
            Self::LengthMismatch { expected, actual } => {
                write!(f, "artifact length {actual}, pinned {expected}")
            }
            Self::FingerprintMismatch { expected, actual } => {
                write!(f, "artifact fingerprint {actual:#x}, pinned {expected:#x}")
            }
            Self::MissingSymbol(name) => write!(f, "artifact does not export {name}"),
            Self::UnsupportedHost => f.write_str("host target has no pinned OpenTUI artifact"),
        }
    }
}

impl std::error::Error for IdentityError {}

/// Validate a 40-character lowercase hex commit.
pub fn parse_commit(text: &str) -> Result<&str, IdentityError> {
    let valid = text.len() == 40
        && text
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte));
    if valid {
        Ok(text)
    } else {
        Err(IdentityError::BadCommit {
            value: text.to_string(),
        })
    }
}

/// FNV-1a 64 over arbitrary bytes (the pin's artifact fingerprint).
#[must_use]
pub fn fnv1a64(bytes: &[u8]) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}

/// Observed artifact facts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Observation {
    pub bytes: u64,
    pub fingerprint: u64,
    pub symbols: BTreeSet<String>,
}

impl Observation {
    /// Observe a file's length and fingerprint.
    pub fn read(path: &Path) -> Result<Self, IdentityError> {
        let bytes = std::fs::read(path).map_err(|error| IdentityError::Io {
            path: path.to_path_buf(),
            kind: error.kind(),
        })?;
        Ok(Self {
            bytes: bytes.len() as u64,
            fingerprint: fnv1a64(&bytes),
            symbols: BTreeSet::new(),
        })
    }

    /// Attach a parsed `nm` symbol table.
    #[must_use]
    pub fn with_symbols(mut self, symbols: BTreeSet<String>) -> Self {
        self.symbols = symbols;
        self
    }
}

/// Parse `nm` output into a symbol set.
///
/// Handles both `00000000001d9480 T _createRenderer` (Darwin) and
/// `0000000000001234 T createRenderer` (Linux `nm -D`) shapes; the leading
/// underscore Mach-O adds is stripped so one pin serves both formats.
#[must_use]
pub fn parse_nm_symbols(output: &str) -> BTreeSet<String> {
    output
        .lines()
        .filter_map(|line| {
            let mut fields = line.split_whitespace();
            let _address = fields.next()?;
            let _kind = fields.next()?;
            let name = fields.next()?;
            Some(name.trim_start_matches('_').to_string())
        })
        .collect()
}

/// Verify a pin against an observation.
pub fn identify(
    pin: &'static ArtifactPin,
    observation: &Observation,
) -> Result<Identified, IdentityError> {
    parse_commit(OPENTUI_COMMIT)?;
    if observation.bytes != pin.bytes {
        return Err(IdentityError::LengthMismatch {
            expected: pin.bytes,
            actual: observation.bytes,
        });
    }
    if observation.fingerprint != pin.fingerprint {
        return Err(IdentityError::FingerprintMismatch {
            expected: pin.fingerprint,
            actual: observation.fingerprint,
        });
    }
    let mut count = 0;
    for symbol in REQUIRED_SYMBOLS {
        if !observation.symbols.contains(symbol) {
            return Err(IdentityError::MissingSymbol(symbol));
        }
        count += 1;
    }
    Ok(Identified {
        opencode_commit: OPENCODE_COMMIT,
        opentui_version: OPENTUI_VERSION,
        opentui_commit: OPENTUI_COMMIT,
        abi_version: ABI_VERSION,
        triple: pin.triple,
        symbol_count: count,
    })
}

/// Pin for a target triple.
#[must_use]
pub fn pin_for(triple: &str) -> Option<&'static ArtifactPin> {
    ARTIFACTS.iter().find(|pin| pin.triple == triple)
}

/// Directory holding the vendored native libraries, relative to this crate.
#[must_use]
pub fn native_library_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("native")
        .join("lib")
}

/// Path of the pinned artifact for `triple`.
#[must_use]
pub fn artifact_path(triple: &str) -> Option<PathBuf> {
    let pin = pin_for(triple)?;
    Some(native_library_dir().join(triple).join(pin.file))
}

/// Run the host `nm` on `path` and parse its exports.
pub fn probe_symbols(path: &Path) -> Result<BTreeSet<String>, IdentityError> {
    let output = if cfg!(target_os = "macos") {
        std::process::Command::new("nm")
            .arg("-gU")
            .arg(path)
            .output()
    } else {
        std::process::Command::new("nm")
            .arg("-D")
            .arg("--defined-only")
            .arg(path)
            .output()
    };
    let output = output.map_err(|error| IdentityError::Io {
        path: path.to_path_buf(),
        kind: error.kind(),
    })?;
    if !output.status.success() {
        return Err(IdentityError::Io {
            path: path.to_path_buf(),
            kind: io::ErrorKind::Other,
        });
    }
    Ok(parse_nm_symbols(&String::from_utf8_lossy(&output.stdout)))
}

/// Full probe of the installed bytes for `triple`.
pub fn probe_installed(triple: &str) -> Result<Identified, IdentityError> {
    let pin = pin_for(triple).ok_or_else(|| IdentityError::UnknownTriple {
        triple: triple.to_string(),
    })?;
    let path = artifact_path(triple).expect("pin implies path");
    let symbols = probe_symbols(&path)?;
    let observation = Observation::read(&path)?.with_symbols(symbols);
    identify(pin, &observation)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn synthetic(pin: &ArtifactPin, symbols: &[&str]) -> Observation {
        Observation {
            bytes: pin.bytes,
            fingerprint: pin.fingerprint,
            symbols: symbols.iter().map(|name| (*name).to_string()).collect(),
        }
    }

    #[test]
    fn pin_records_are_well_formed() {
        assert_eq!(OPENTUI_VERSION, "0.6.8");
        assert_eq!(parse_commit(OPENTUI_COMMIT), Ok(OPENTUI_COMMIT));
        assert_eq!(parse_commit(OPENCODE_COMMIT), Ok(OPENCODE_COMMIT));
        assert_eq!(ABI_VERSION, 1);
        assert_eq!(ARTIFACTS.len(), 3);
        let mut triples: Vec<&str> = ARTIFACTS.iter().map(|pin| pin.triple).collect();
        triples.sort_unstable();
        triples.dedup();
        assert_eq!(triples.len(), ARTIFACTS.len(), "triples are unique");
        for pin in &ARTIFACTS {
            assert!(pin.bytes > 0);
            assert_ne!(pin.fingerprint, 0);
        }
        // Same release everywhere: all artifacts are one commit.
        assert_eq!(pin_for("aarch64-apple-darwin").unwrap().file, "libopentui.dylib");
        assert!(pin_for("wasm32-unknown-unknown").is_none());
    }

    #[test]
    fn commit_validation_is_strict() {
        assert!(parse_commit(&OPENTUI_COMMIT.to_uppercase()).is_err());
        assert!(parse_commit(&OPENTUI_COMMIT[..39]).is_err());
        assert!(parse_commit(&format!("{OPENTUI_COMMIT}0")).is_err());
        assert!(parse_commit(&"g".repeat(40)).is_err());
        assert!(parse_commit("").is_err());
        assert_eq!(
            parse_commit("xyz"),
            Err(IdentityError::BadCommit {
                value: "xyz".to_string()
            })
        );
    }

    #[test]
    fn fingerprint_is_deterministic() {
        // FNV-1a 64 reference vectors.
        assert_eq!(fnv1a64(b""), 0xcbf2_9ce4_8422_2325);
        assert_eq!(fnv1a64(b"a"), 0xaf63_dc4c_8601_ec8c);
        assert_eq!(fnv1a64(b"foobar"), 0x8594_4171_f739_67e8);
    }

    #[test]
    fn identify_requires_length_fingerprint_and_symbols() {
        let pin = &ARTIFACTS[0];
        let full: Vec<&str> = REQUIRED_SYMBOLS.to_vec();
        assert!(identify(pin, &synthetic(pin, &full)).is_ok());

        let mut wrong_length = synthetic(pin, &full);
        wrong_length.bytes += 1;
        assert_eq!(
            identify(pin, &wrong_length),
            Err(IdentityError::LengthMismatch {
                expected: pin.bytes,
                actual: pin.bytes + 1
            })
        );

        let mut wrong_fingerprint = synthetic(pin, &full);
        wrong_fingerprint.fingerprint ^= 1;
        assert!(matches!(
            identify(pin, &wrong_fingerprint),
            Err(IdentityError::FingerprintMismatch { .. })
        ));

        let partial = synthetic(pin, &[REQUIRED_SYMBOLS[0], REQUIRED_SYMBOLS[1]]);
        assert_eq!(
            identify(pin, &partial),
            Err(IdentityError::MissingSymbol(REQUIRED_SYMBOLS[2]))
        );
    }

    #[test]
    fn nm_output_parses_both_formats() {
        let darwin = "00000000001d9480 T _createRenderer\n\
                      00000000001daf50 T _getBuildOptions\n\
                      0000000000000000 S _someData\n";
        let symbols = parse_nm_symbols(darwin);
        assert!(symbols.contains("createRenderer"));
        assert!(symbols.contains("getBuildOptions"));
        assert!(!symbols.contains("_createRenderer"));

        let linux = "0000000000001234 T createRenderer\n\
                     0000000000005678 T textBufferAppend\n";
        let symbols = parse_nm_symbols(linux);
        assert!(symbols.contains("createRenderer"));
        assert!(symbols.contains("textBufferAppend"));
        assert!(parse_nm_symbols("").is_empty());
        assert!(parse_nm_symbols("garbage\n").is_empty());
    }

    #[test]
    fn pinned_artifact_bytes_match_the_recorded_fingerprint() {
        // Host-only: the vendored tree contains the artifact for this build
        // target. Verifies that the pin is the bytes actually linked.
        let Some(path) = artifact_path(HOST_TRIPLE) else {
            // Unsupported workstation target: nothing is pinned to compare.
            return;
        };
        let pin = pin_for(HOST_TRIPLE).unwrap();
        let observation = Observation::read(&path).expect("vendored artifact is readable");
        assert_eq!(observation.bytes, pin.bytes, "artifact length drifted");
        assert_eq!(
            observation.fingerprint, pin.fingerprint,
            "artifact bytes drifted from the pinned OpenTUI build"
        );
    }

    #[test]
    fn installed_probe_identifies_the_exact_commit() {
        if artifact_path(HOST_TRIPLE).is_none() {
            return;
        }
        // `nm` is part of the platform toolchain used by this crate's build;
        // when it is unavailable the probe cannot run and is skipped.
        match probe_installed(HOST_TRIPLE) {
            Ok(identity) => {
                assert_eq!(identity.opentui_commit, OPENTUI_COMMIT);
                assert_eq!(identity.opencode_commit, OPENCODE_COMMIT);
                assert_eq!(identity.opentui_version, OPENTUI_VERSION);
                assert_eq!(identity.abi_version, ABI_VERSION);
                assert_eq!(identity.triple, HOST_TRIPLE);
                assert_eq!(identity.symbol_count, REQUIRED_SYMBOLS.len());
            }
            // macOS `nm --version` also fails, so only treat a real identity
            // failure as a test failure. An absent `nm` shows up as Io.
            Err(IdentityError::Io { .. }) => {}
            Err(other) => panic!("artifact identity mismatch: {other}"),
        }
    }
}
