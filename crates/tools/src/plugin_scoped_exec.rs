//! EXT-006 broker-gated scoped plugin execution: capability check then broker
//! check then one bounded in-memory effect. Pure, no I/O, no threads, no
//! registry, no JS host. Caller owns grant, broker, sink, cancel.

use std::fmt;
use std::sync::atomic::{AtomicBool, Ordering};
use thiserror::Error;

/// Max `EffectRequest.args` bytes copied into a sink entry / outcome.
pub const MAX_ARGS_BYTES: usize = 4096;
/// Max `EffectOutcome.bytes_out` length.
pub const MAX_RESULT_BYTES: usize = 4096;
/// Max entries retained in one caller-owned [`EffectSink`].
pub const MAX_SINK_ENTRIES: usize = 256;
/// Max total retained bytes (`key.len() + value.len()` summed) in one sink.
pub const MAX_SINK_BYTES: u64 = 65536;
/// Max capabilities bound into one [`PluginGrant`] (EXT-005 cap).
pub const MAX_GRANT_CAPABILITIES: usize = 16;
/// Max length of one capability label or op name.
pub const MAX_CAP_LEN: usize = 64;
/// Max plugin id length.
pub const MAX_PLUGIN_ID_LEN: usize = 128;
/// Manifest contract version this lane accepts (must equal EXT-005 value).
pub const SUPPORTED_CONTRACT_VERSION: u32 = 1;

/// Caller-submitted manifest declaration (EXT-005 shape), revalidated at bind.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Manifest {
    pub name: String,
    pub contract_version: u32,
    pub capabilities: Vec<String>,
}

/// Validated manifest capabilities snapshotted to one registry id at bind time.
/// Later manifest edits never retro-apply; revocation is caller-driven.
#[derive(Clone, PartialEq, Eq)]
pub struct PluginGrant {
    pub plugin: String,
    pub capabilities: Vec<String>,
    pub revoked: bool,
}

impl fmt::Debug for PluginGrant {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PluginGrant")
            .field("plugin", &self.plugin)
            .field("capabilities", &self.capabilities)
            .field("revoked", &self.revoked)
            .finish()
    }
}

impl PluginGrant {
    /// Caller-driven revocation (no background watcher): the caller drops or
    /// marks the grant when the plugin id leaves the EXT-001 registry.
    pub fn revoke(&mut self) {
        self.revoked = true;
    }
}

/// One bounded native effect invocation under a granted capability.
#[derive(Clone, PartialEq, Eq)]
pub struct EffectRequest {
    pub capability: String,
    pub op: String,
    pub args: Vec<u8>,
}

impl fmt::Debug for EffectRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("EffectRequest")
            .field("capability", &self.capability)
            .field("op", &self.op)
            .field("args_len", &self.args.len())
            .finish()
    }
}

/// Native in-memory outcome. Byte payloads are opaque and never logged.
#[derive(Clone, PartialEq, Eq)]
pub struct EffectOutcome {
    pub applied: bool,
    pub bytes_out: Vec<u8>,
}

impl fmt::Debug for EffectOutcome {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("EffectOutcome")
            .field("applied", &self.applied)
            .field("bytes_out_len", &self.bytes_out.len())
            .finish()
    }
}

/// Every `execute` failure mode. Deny/validation errors never mutate the sink.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ExecError {
    #[error("unknown capability")]
    UnknownCapability,
    #[error("invalid op")]
    InvalidOp,
    #[error("args too large")]
    TooLarge,
    #[error("cancelled")]
    Cancelled,
    #[error("denied by broker")]
    Denied,
    #[error("sink overflow")]
    Overflow,
    #[error("grant revoked")]
    Revoked,
    #[error("invalid grant")]
    InvalidGrant,
}

/// Caller-supplied trusted permission broker. No default-allow impl ships:
/// every invocation asserts a grant first, and `*` never bypasses mandatory
/// human-only/system protections (enforced by the caller's broker).
pub trait PermissionBroker {
    fn assert(&self, req: &EffectRequest) -> bool;
}

/// Caller-owned bounded in-memory map. Key is `{capability}:{op}`.
/// Total retained bytes = sum of `key.len() + value.len()`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct EffectSink {
    entries: Vec<(String, Vec<u8>)>,
}

impl EffectSink {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn get(&self, key: &str) -> Option<&[u8]> {
        self.entries
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.as_slice())
    }

    /// Total retained bytes, saturating.
    pub fn retained_bytes(&self) -> u64 {
        self.entries.iter().fold(0u64, |acc, (k, v)| {
            acc.saturating_add(k.len() as u64)
                .saturating_add(v.len() as u64)
        })
    }

    /// Deterministic content hash for deny-no-side-effect assertions.
    pub fn fingerprint(&self) -> u64 {
        let mut h: u64 = 0xcbf29ce484222325;
        for (k, v) in &self.entries {
            for b in k.as_bytes().iter().chain(v.iter()) {
                h ^= u64::from(*b);
                h = h.wrapping_mul(0x100000001b3);
            }
            h = h.wrapping_mul(0x100000001b3) ^ (v.len() as u64);
        }
        h
    }
}

fn charset_ok(s: &str) -> bool {
    let mut bytes = s.bytes();
    match bytes.next() {
        Some(b) if b.is_ascii_alphanumeric() => (),
        _ => return false,
    }
    s.bytes()
        .all(|b| b.is_ascii_alphanumeric() || b == b'.' || b == b'_' || b == b'-')
}

fn cap_ok(s: &str) -> bool {
    !s.is_empty() && s.len() <= MAX_CAP_LEN && charset_ok(s)
}

fn op_ok(s: &str) -> bool {
    cap_ok(s)
}

fn name_ok(s: &str) -> bool {
    !s.is_empty() && s.len() <= MAX_PLUGIN_ID_LEN && charset_ok(s)
}

/// Revalidate `manifest` (EXT-005 rules) and snapshot its capability list onto
/// `plugin_id`. Later manifest edits never retro-apply.
pub fn bind(plugin_id: &str, manifest: &Manifest) -> Result<PluginGrant, ExecError> {
    if !name_ok(plugin_id) || !name_ok(&manifest.name) {
        return Err(ExecError::InvalidGrant);
    }
    if manifest.contract_version != SUPPORTED_CONTRACT_VERSION {
        return Err(ExecError::InvalidGrant);
    }
    if manifest.capabilities.len() > MAX_GRANT_CAPABILITIES {
        return Err(ExecError::InvalidGrant);
    }
    let mut seen: Vec<&str> = Vec::with_capacity(manifest.capabilities.len());
    for c in &manifest.capabilities {
        if !cap_ok(c) || seen.contains(&c.as_str()) {
            return Err(ExecError::InvalidGrant);
        }
        seen.push(c.as_str());
    }
    Ok(PluginGrant {
        plugin: plugin_id.to_string(),
        capabilities: manifest.capabilities.clone(),
        revoked: false,
    })
}

/// Capability membership, op shape, args bound, cancel, broker assertion, then
/// one bounded in-memory write. Any early failure leaves the sink untouched;
/// broker deny provably mutates nothing.
pub fn execute(
    grant: &PluginGrant,
    req: &EffectRequest,
    broker: &dyn PermissionBroker,
    sink: &mut EffectSink,
    cancel: &AtomicBool,
) -> Result<EffectOutcome, ExecError> {
    if grant.revoked {
        return Err(ExecError::Revoked);
    }
    if !grant.capabilities.iter().any(|c| c == &req.capability) {
        return Err(ExecError::UnknownCapability);
    }
    if !op_ok(&req.op) {
        return Err(ExecError::InvalidOp);
    }
    if req.args.len() > MAX_ARGS_BYTES {
        return Err(ExecError::TooLarge);
    }
    if cancel.load(Ordering::SeqCst) {
        return Err(ExecError::Cancelled);
    }
    if !broker.assert(req) {
        return Err(ExecError::Denied);
    }
    if cancel.load(Ordering::SeqCst) {
        return Err(ExecError::Cancelled);
    }
    let key = format!("{}:{}", req.capability, req.op);
    let value = req.args.clone();
    if value.len() > MAX_RESULT_BYTES {
        return Err(ExecError::TooLarge);
    }
    let entry_bytes = (key.len() as u64).saturating_add(value.len() as u64);
    match sink.entries.iter().position(|(k, _)| k == &key) {
        Some(i) => {
            let old =
                (sink.entries[i].0.len() as u64).saturating_add(sink.entries[i].1.len() as u64);
            let next = sink
                .retained_bytes()
                .saturating_sub(old)
                .saturating_add(entry_bytes);
            if next > MAX_SINK_BYTES {
                return Err(ExecError::Overflow);
            }
            sink.entries[i].1 = value.clone();
        }
        None => {
            if sink.entries.len() >= MAX_SINK_ENTRIES {
                return Err(ExecError::Overflow);
            }
            if sink.retained_bytes().saturating_add(entry_bytes) > MAX_SINK_BYTES {
                return Err(ExecError::Overflow);
            }
            sink.entries.push((key, value.clone()));
        }
    }
    Ok(EffectOutcome {
        applied: true,
        bytes_out: value,
    })
}
