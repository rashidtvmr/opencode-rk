//! Bounded, fail-closed projection of MCP servers and tools.
//!
//! This module carries identity labels and enablement state only. It never
//! starts a server, reads configuration, resolves credentials, or performs
//! network or filesystem I/O. The filtered value owns a deterministic
//! integrity snapshot so callers can reject stale or tampered projections
//! before using a tool reference.

use std::borrow::Borrow;
use std::fmt;

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Maximum MCP servers admitted to one payload.
pub const MAX_SERVERS: usize = 64;
/// Maximum MCP tools admitted to one payload.
pub const MAX_TOOLS: usize = 512;
/// Maximum length of a server or tool identity, measured in Unicode scalar
/// values. Identity values are copied only after this bound is checked.
pub const MAX_FIELD_CHARS: usize = 128;

const FNV_OFFSET: u64 = 0xcbf29ce484222325;
const FNV_PRIME: u64 = 0x100000001b3;

/// Caller-owned MCP server enablement view.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ServerFlag {
    pub id: String,
    pub enabled: bool,
}

/// Caller-owned MCP tool identity. It contains no schema, arguments, or
/// handler state.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ToolEntry {
    pub server_id: String,
    pub name: String,
}

/// Caller-owned input to [`filter_payload`].
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct PayloadInput {
    pub servers: Vec<ServerFlag>,
    pub tools: Vec<ToolEntry>,
}

/// Safe tool identity exposed after server enablement filtering.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
pub struct ToolRef {
    pub server_id: String,
    pub name: String,
}

/// Filtered MCP payload and its deterministic integrity snapshot.
///
/// `disabled_servers` and `integrity_hash` are private implementation state.
/// They are excluded from serialization so the wire shape contains only the
/// prescribed enabled server ids, tool references, and snapshot hash.
#[derive(Clone, Eq, PartialEq, Serialize)]
pub struct FilteredPayload {
    pub servers: Vec<String>,
    pub tools: Vec<ToolRef>,
    pub snapshot_hash: u64,
    #[serde(skip)]
    disabled_servers: Vec<String>,
    #[serde(skip)]
    integrity_hash: u64,
}

impl fmt::Debug for FilteredPayload {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FilteredPayload")
            .field("servers", &self.servers)
            .field("tools", &self.tools)
            .field("snapshot_hash", &self.snapshot_hash)
            .finish()
    }
}

/// Failures are deliberately payload-independent: no input label is copied
/// into an error that might be logged by a caller.
#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum PayloadError {
    #[error("payload field is empty")]
    EmptyField,
    #[error("payload exceeds its bound")]
    OverCap,
    #[error("MCP server is disabled")]
    NotEnabled,
    #[error("unknown MCP server or tool")]
    Unknown,
}

fn validate_field(value: &str) -> Result<(), PayloadError> {
    if value.is_empty() {
        return Err(PayloadError::EmptyField);
    }
    if value.chars().nth(MAX_FIELD_CHARS).is_some() {
        return Err(PayloadError::OverCap);
    }
    Ok(())
}

fn validate_input(input: &PayloadInput) -> Result<(), PayloadError> {
    // Check collection caps before inspecting or projecting any item. This
    // makes oversized input fail as one transaction with no partial result.
    if input.servers.len() > MAX_SERVERS || input.tools.len() > MAX_TOOLS {
        return Err(PayloadError::OverCap);
    }
    for server in &input.servers {
        validate_field(&server.id)?;
    }
    for tool in &input.tools {
        validate_field(&tool.server_id)?;
        validate_field(&tool.name)?;
    }
    Ok(())
}

fn sort_dedup(values: &mut Vec<String>) {
    values.sort_unstable();
    values.dedup();
}

fn fnv_bytes(mut hash: u64, bytes: &[u8]) -> u64 {
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    hash
}

fn fnv_len(hash: u64, len: usize) -> u64 {
    // All admitted labels are <= 128 chars, but a fixed-width u64 length
    // prefix keeps the encoding deterministic across target architectures.
    fnv_bytes(hash, (len as u64).to_le_bytes().as_slice())
}

fn fnv_label(hash: u64, label: &str) -> u64 {
    let hash = fnv_len(hash, label.len());
    fnv_bytes(hash, label.as_bytes())
}

/// Hash the complete canonical projection. Server ids are included so a
/// no-tool enabled server cannot be silently added or removed without making
/// snapshot verification fail; each tool remains an ordered `(server_id,
/// name)` pair in the encoded stream.
fn payload_hash(servers: &[String], tools: &[ToolRef]) -> u64 {
    let mut hash = FNV_OFFSET;
    hash = fnv_bytes(hash, b"mcp-payload-v1\0");
    hash = fnv_len(hash, servers.len());
    for server in servers {
        hash = fnv_bytes(hash, b"S");
        hash = fnv_label(hash, server);
    }
    hash = fnv_len(hash, tools.len());
    for tool in tools {
        hash = fnv_bytes(hash, b"T");
        hash = fnv_label(hash, &tool.server_id);
        hash = fnv_label(hash, &tool.name);
    }
    hash
}

fn valid_projection(filtered: &FilteredPayload) -> bool {
    if filtered.servers.len() > MAX_SERVERS
        || filtered.tools.len() > MAX_TOOLS
        || filtered.disabled_servers.len() > MAX_SERVERS
    {
        return false;
    }
    if filtered
        .servers
        .iter()
        .any(|id| validate_field(id).is_err())
        || filtered
            .disabled_servers
            .iter()
            .any(|id| validate_field(id).is_err())
        || filtered.tools.iter().any(|tool| {
            validate_field(&tool.server_id).is_err() || validate_field(&tool.name).is_err()
        })
    {
        return false;
    }
    filtered.servers.windows(2).all(|pair| pair[0] < pair[1])
        && filtered
            .disabled_servers
            .windows(2)
            .all(|pair| pair[0] < pair[1])
        && filtered.tools.windows(2).all(|pair| pair[0] <= pair[1])
}

/// Filter a caller-owned MCP view to enabled servers and their tools.
///
/// Accepts either `PayloadInput` or `&PayloadInput` for ergonomic use by
/// callers. Validation completes before any output is built. Duplicate server
/// flags are collapsed; if the same id is both enabled and disabled, disabled
/// wins. Tool entries are retained as supplied, then sorted by server id and
/// tool name so repeated filtering is deterministic.
pub fn filter_payload<T>(input: T) -> Result<FilteredPayload, PayloadError>
where
    T: Borrow<PayloadInput>,
{
    let input = input.borrow();
    validate_input(input)?;

    let mut enabled_servers = Vec::with_capacity(input.servers.len());
    let mut disabled_servers = Vec::with_capacity(input.servers.len());
    for server in &input.servers {
        if server.enabled {
            enabled_servers.push(server.id.clone());
        } else {
            disabled_servers.push(server.id.clone());
        }
    }
    sort_dedup(&mut enabled_servers);
    sort_dedup(&mut disabled_servers);

    // A disabled observation always wins over an enabled observation. This is
    // the fail-closed rule for conflicting or stale registry views.
    enabled_servers.retain(|id| disabled_servers.binary_search(id).is_err());

    let mut tools = Vec::with_capacity(input.tools.len());
    for tool in &input.tools {
        if enabled_servers.binary_search(&tool.server_id).is_ok() {
            tools.push(ToolRef {
                server_id: tool.server_id.clone(),
                name: tool.name.clone(),
            });
        }
    }
    tools.sort_unstable();

    let snapshot_hash = payload_hash(&enabled_servers, &tools);
    Ok(FilteredPayload {
        servers: enabled_servers,
        tools,
        snapshot_hash,
        disabled_servers,
        integrity_hash: snapshot_hash,
    })
}

/// Look up an enabled tool from a previously filtered payload.
///
/// The integrity check runs before lookup. Public projection fields are thus
/// not trusted after mutation, and a stale or tampered projection fails closed
/// as [`PayloadError::Unknown`].
pub fn lookup_tool<T>(filtered: T, server_id: &str, name: &str) -> Result<ToolRef, PayloadError>
where
    T: Borrow<FilteredPayload>,
{
    let filtered = filtered.borrow();
    validate_field(server_id)?;
    validate_field(name)?;

    if !valid_projection(filtered)
        || filtered.snapshot_hash != filtered.integrity_hash
        || payload_hash(&filtered.servers, &filtered.tools) != filtered.integrity_hash
    {
        return Err(PayloadError::Unknown);
    }
    if filtered
        .disabled_servers
        .binary_search_by(|id| id.as_str().cmp(server_id))
        .is_ok()
    {
        return Err(PayloadError::NotEnabled);
    }
    if filtered
        .servers
        .binary_search_by(|id| id.as_str().cmp(server_id))
        .is_err()
    {
        return Err(PayloadError::Unknown);
    }

    filtered
        .tools
        .binary_search_by(|tool| {
            (tool.server_id.as_str(), tool.name.as_str()).cmp(&(server_id, name))
        })
        .map(|index| filtered.tools[index].clone())
        .map_err(|_| PayloadError::Unknown)
}

/// Recompute and verify a filtered payload's deterministic snapshot.
#[must_use]
pub fn verify_snapshot(filtered: &FilteredPayload, expected_hash: u64) -> bool {
    valid_projection(filtered)
        && expected_hash == filtered.snapshot_hash
        && expected_hash == filtered.integrity_hash
        && payload_hash(&filtered.servers, &filtered.tools) == expected_hash
}
