//! External manifest validation (EXT-005): bounded bytes in, typed verdict out.
//!
//! Pure function: no registry, no I/O, no threads, no global state. Caller
//! owns input bytes and the returned [`Manifest`]. Check order: length cap,
//! UTF-8, JSON parse, field validation. Unknown JSON fields ignored.
//!
//! Lane-owned module: included by the test via `#[path]`; the integrator
//! wires `pub mod ext_manifest_lane;` into `lib.rs` later. No dependency on
//! other crate modules. Distinct from `plugin_manifest` (name/version/
//! permissions shape): this is the EXT-005 contract (name/contract_version/
//! capabilities over raw JSON bytes).

use thiserror::Error;

/// Only contract version accepted by [`validate_manifest_bytes`].
pub const SUPPORTED_CONTRACT_VERSION: u32 = 1;
/// Maximum manifest input bytes; checked before parsing.
pub const MAX_MANIFEST_BYTES: usize = 16384;
/// Maximum manifest name length in bytes.
pub const MAX_NAME_LEN: usize = 128;
/// Maximum capabilities per manifest.
pub const MAX_CAPABILITIES: usize = 16;
/// Maximum single-capability length in bytes.
pub const MAX_CAP_LEN: usize = 64;

/// Validated manifest: name + contract version + capability labels only.
/// Holds labels, never file bodies or secrets.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Manifest {
    pub name: String,
    pub contract_version: u32,
    pub capabilities: Vec<String>,
}

/// Typed manifest verdicts. Pure values; no state exists to mutate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum ManifestError {
    #[error("manifest exceeds 16384 bytes")]
    TooLarge,
    #[error("manifest is not valid UTF-8")]
    InvalidEncoding,
    #[error("manifest is not a JSON object with the expected shape")]
    InvalidJson,
    #[error("invalid manifest name")]
    InvalidName,
    #[error("invalid capabilities")]
    InvalidCapabilities,
    #[error("unsupported contract version")]
    UnsupportedContract,
}

fn label_ok(s: &str, max_len: usize) -> bool {
    if s.is_empty() || s.len() > max_len {
        return false;
    }
    let mut bytes = s.bytes();
    match bytes.next() {
        Some(b) if b.is_ascii_alphanumeric() => {}
        _ => return false,
    }
    bytes.all(|b| b.is_ascii_alphanumeric() || b == b'.' || b == b'_' || b == b'-')
}

/// Struct-level checks without parsing: name, contract version, then
/// capabilities (count, entry shape, intra-manifest duplicates).
pub fn validate_manifest(m: &Manifest) -> Result<(), ManifestError> {
    if !label_ok(&m.name, MAX_NAME_LEN) {
        return Err(ManifestError::InvalidName);
    }
    if m.contract_version != SUPPORTED_CONTRACT_VERSION {
        return Err(ManifestError::UnsupportedContract);
    }
    if m.capabilities.len() > MAX_CAPABILITIES {
        return Err(ManifestError::InvalidCapabilities);
    }
    for (i, c) in m.capabilities.iter().enumerate() {
        if !label_ok(c, MAX_CAP_LEN) {
            return Err(ManifestError::InvalidCapabilities);
        }
        if m.capabilities[..i].contains(c) {
            return Err(ManifestError::InvalidCapabilities);
        }
    }
    Ok(())
}

/// Validate bounded manifest bytes. Steps in order: length cap, UTF-8
/// check, JSON parse (`{name, contract_version, capabilities?}`; unknown
/// fields ignored), field validation. Missing `capabilities` defaults
/// to `[]`. Never panics; allocates only the returned struct.
pub fn validate_manifest_bytes(bytes: &[u8]) -> Result<Manifest, ManifestError> {
    if bytes.len() > MAX_MANIFEST_BYTES {
        return Err(ManifestError::TooLarge);
    }
    let text = std::str::from_utf8(bytes).map_err(|_| ManifestError::InvalidEncoding)?;
    let value: serde_json::Value =
        serde_json::from_str(text).map_err(|_| ManifestError::InvalidJson)?;
    let obj = value.as_object().ok_or(ManifestError::InvalidJson)?;
    let name = match obj.get("name") {
        Some(serde_json::Value::String(s)) => s.as_str(),
        Some(_) | None => return Err(ManifestError::InvalidName),
    };
    if !label_ok(name, MAX_NAME_LEN) {
        return Err(ManifestError::InvalidName);
    }
    let contract_version = match obj.get("contract_version") {
        Some(serde_json::Value::Number(n)) => match n.as_u64() {
            Some(v) if v == u64::from(SUPPORTED_CONTRACT_VERSION) => SUPPORTED_CONTRACT_VERSION,
            _ => return Err(ManifestError::UnsupportedContract),
        },
        _ => return Err(ManifestError::UnsupportedContract),
    };
    let capabilities: Vec<String> = match obj.get("capabilities") {
        None => Vec::new(),
        Some(serde_json::Value::Array(items)) => {
            if items.len() > MAX_CAPABILITIES {
                return Err(ManifestError::InvalidCapabilities);
            }
            let mut out = Vec::with_capacity(items.len());
            for item in items {
                match item {
                    serde_json::Value::String(s) => {
                        if !label_ok(s, MAX_CAP_LEN) || out.contains(s) {
                            return Err(ManifestError::InvalidCapabilities);
                        }
                        out.push(s.clone());
                    }
                    _ => return Err(ManifestError::InvalidCapabilities),
                }
            }
            out
        }
        Some(_) => return Err(ManifestError::InvalidCapabilities),
    };
    let manifest = Manifest {
        name: name.to_string(),
        contract_version,
        capabilities,
    };
    validate_manifest(&manifest)?;
    Ok(manifest)
}
