//! OPS-004 pure install version/channel metadata + derived runtime paths.
//!
//! Offline only: parses `"<version>-<channel>"` strings and derives
//! XDG-style paths from caller-supplied base dirs. No filesystem mutation,
//! no process-environment read, no network, no clock, no global state.

/// Install channel.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Channel {
    Stable,
    Dev,
    Custom(String),
}

/// Parsed version metadata.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VersionMeta {
    pub version: String,
    pub channel: Channel,
}

/// Caller-supplied base dirs (already validated upstream; re-checked here).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BaseDirs {
    pub data_dir: String,
    pub cache_dir: String,
    pub config_dir: String,
}

/// Derived runtime paths (construction only, nothing created).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DerivedPaths {
    pub data_dir: String,
    pub cache_dir: String,
    pub config_dir: String,
    pub version_file: String,
}

/// Parse failure kinds. Only the kind is ever surfaced; inputs are not logged.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VersionError {
    Invalid,
    UnknownChannel,
    Unsafe,
    BadBaseDir,
}

const MAX_RAW: usize = 128;
const MAX_BASE: usize = 1024;
const MAX_DERIVED: usize = 2048;
const MAX_LABEL: usize = 64;

fn is_path_unsafe(s: &str) -> bool {
    s.contains("..") || s.contains('/') || s.contains('\\') || s.contains('\0')
}

fn is_token_char(b: u8) -> bool {
    b.is_ascii_alphanumeric() || matches!(b, b'.' | b'-' | b'_' | b'+')
}

/// Parse `"<version>-stable"`, `"<version>-dev"`, or
/// `"<version>-custom: <label>"` into [`VersionMeta`].
pub fn parse_version(raw: &str) -> Result<VersionMeta, VersionError> {
    if raw.is_empty() || raw.len() > MAX_RAW {
        return Err(VersionError::Invalid);
    }
    if raw.len() != raw.trim().len() {
        return Err(VersionError::Invalid);
    }
    if is_path_unsafe(raw) {
        return Err(VersionError::Unsafe);
    }
    let (version, channel) = if let Some(v) = raw.strip_suffix("-stable") {
        (v, Channel::Stable)
    } else if let Some(v) = raw.strip_suffix("-dev") {
        (v, Channel::Dev)
    } else if let Some(idx) = raw.find("-custom:") {
        let (v, rest) = raw.split_at(idx);
        let label = rest["-custom:".len()..].trim();
        if label.is_empty() || label.len() > MAX_LABEL {
            return Err(VersionError::Invalid);
        }
        if !label.bytes().all(is_token_char) {
            return Err(VersionError::Invalid);
        }
        (v, Channel::Custom(label.to_string()))
    } else {
        return Err(VersionError::UnknownChannel);
    };
    if version.is_empty() || !version.bytes().all(is_token_char) {
        return Err(VersionError::Invalid);
    }
    Ok(VersionMeta {
        version: version.to_string(),
        channel,
    })
}

/// Derive runtime paths from parsed metadata and caller base dirs.
/// Channel is folded into `cache_dir` so channels never share cache state.
pub fn derive_paths(meta: &VersionMeta, base: &BaseDirs) -> Result<DerivedPaths, VersionError> {
    for dir in [&base.data_dir, &base.cache_dir, &base.config_dir] {
        if dir.is_empty() || dir.len() > MAX_BASE || dir.contains('\0') {
            return Err(VersionError::BadBaseDir);
        }
    }
    let cache_dir = match &meta.channel {
        Channel::Stable => base.cache_dir.clone(),
        Channel::Dev => format!("{}-dev", base.cache_dir),
        Channel::Custom(label) => format!("{}-custom-{}", base.cache_dir, label),
    };
    let version_file = format!("{}/installation/version", base.data_dir);
    let out = DerivedPaths {
        data_dir: base.data_dir.clone(),
        cache_dir,
        config_dir: base.config_dir.clone(),
        version_file,
    };
    for p in [&out.data_dir, &out.cache_dir, &out.config_dir, &out.version_file] {
        if p.len() > MAX_DERIVED {
            return Err(VersionError::BadBaseDir);
        }
    }
    Ok(out)
}
