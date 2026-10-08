//! Authenticated MCP status projection and bounded lifecycle backing.
//!
//! Wire contract mirrors the pinned upstream at
//! `95daf90670b7c039c436c85537da5fbfe2205b41`
//! (`packages/opencode/src/server/routes/instance/httpapi/{groups/mcp.ts,
//! handlers/mcp.ts}` plus `packages/opencode/src/mcp/index.ts`): status is a
//! **name-keyed map** of typed statuses, fresh state is `{}`, add takes
//! `{name, config}` and returns a status map, disconnect returns `true`, and
//! an unknown server is a typed not-found. The local authenticated alias is
//! `GET|POST /api/mcp`; the upstream bare `/mcp` path is deliberately not
//! registered because `daemon_auth::require_bearer` only guards `/api/*`,
//! `/auth/*` and `/events`, so `/mcp` would fall through to the public SPA
//! asset fallback.
//!
//! One registry per router/daemon: `router_with_events` constructs it and
//! injects it as an `Extension`. There is no static global and two routers
//! never share entries.
//!
//! The add boundary is deliberately **strict** and upstream-shaped
//! (`packages/core/src/v1/config/mcp.ts`: discriminated union `Local |
//! Remote` on `type`, `command: String[]`, optional `environment`,
//! optional `enabled`). Only one subset is accepted:
//! `{"type":"local","command":[exe, args...],"enabled":false,
//! "environment"?:{...}}`. Unknown payload/config fields are rejected with
//! 400 so credentials/OAuth material can never be silently discarded;
//! `enabled: true` and an omitted `enabled` (upstream default = enabled)
//! plus `type:"remote"` are stable **501 `mcp_config_unsupported` with no
//! registration** — this build never silently accepts an upstream-enabled
//! request as disabled. Rejected payloads have no side effects.
//!
//! Honest limits of this package: no transport, so `connected`,
//! `needs_auth` and `needs_client_registration` are never produced; configs
//! are validated and held in daemon memory only (no restart durability).

#![forbid(unsafe_code)]

use axum::extract::rejection::JsonRejection;
use axum::extract::{Extension, Json, Path};
use axum::http::StatusCode;
use opencode_rk_tools::mcp_lifecycle::{
    LifecycleError, LifecycleRegistry, LifecycleState, MAX_ERROR_CODE_LEN, MAX_ID_LEN,
};
use serde_json::{json, Value};
use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;
use tokio::sync::Mutex;

use crate::ApiFailure;

/// Hard cap on one `POST /api/mcp` request body, in bytes.
pub(crate) const MAX_ADD_BODY_BYTES: usize = 16 * 1024;

/// Bounds applied to one validated add payload config: the private mirror of
/// the config bounds this classifier enforces (`url` has no bound here
/// because a `url` field is rejected by [`ALLOWED_CONFIG_FIELDS`] before any
/// length check). No public/serde type involved.
struct ConfigBounds {
    max_args: usize,
    max_args_bytes: usize,
    max_env: usize,
    max_env_bytes: usize,
}

const CONFIG_BOUNDS: ConfigBounds = ConfigBounds {
    max_args: 32,
    max_args_bytes: 4096,
    max_env: 32,
    max_env_bytes: 4096,
};

/// Private per-daemon storage for one validated, explicitly disabled local
/// MCP config. Never serialized: `status_map` projects lifecycle state only,
/// so `command`, `args` and `env` never leave this struct — no response,
/// event, or file can contain them.
struct StoredLocalConfig {
    command: String,
    args: Vec<String>,
    env: HashMap<String, String>,
}

/// Stable, secret-free error code emitted when a stored lifecycle code is
/// not safe to echo verbatim.
const INTERNAL_CODE: &str = "internal";
/// Stable code for an entry whose lifecycle flag does not correspond to any
/// validated live connection handle.
const NOT_CONNECTED_CODE: &str = "not_connected";

/// Exact config field set this boundary accepts. Everything else —
/// including upstream-known `cwd`/`timeout`/`headers`/`oauth` — is rejected
/// explicitly (400) so credentials can never be silently discarded.
const ALLOWED_CONFIG_FIELDS: [&str; 4] = ["type", "command", "environment", "enabled"];
/// Exact `POST /api/mcp` payload field set. Extra payload fields are 400.
const ALLOWED_PAYLOAD_FIELDS: [&str; 2] = ["name", "config"];

/// Stable 501 detail: upstream `type:"remote"` config.
const UNSUPPORTED_REMOTE: &str =
    "type \"remote\" is not supported; only explicitly disabled local configs can be registered";
/// Stable 501 detail: explicit `enabled: true`.
const UNSUPPORTED_ENABLED_TRUE: &str =
    "enabled must be false; \"enabled\":true is not supported by this daemon build";
/// Stable 501 detail: omitted `enabled` (upstream default = enabled).
const UNSUPPORTED_ENABLED_DEFAULT: &str =
    "enabled is required and must be false; omitting it means enabled upstream and is not accepted";

#[derive(Default)]
struct RegistryState {
    lifecycle: LifecycleRegistry,
    /// Validated configs, daemon-memory only, never serialized to any
    /// response, event, or file. Contains `env`/`command`, so it must never
    /// leave this struct.
    configs: HashMap<String, StoredLocalConfig>,
}

/// Daemon-owned bounded MCP status registry for one router/instance.
///
/// Every mutation happens here under a short-lived `tokio::sync::Mutex`
/// guard held across in-memory work only — never across I/O and never across
/// a spawned task. Nothing reachable from HTTP can mark an entry
/// `Ready`/`connected`.
#[derive(Clone, Default)]
pub struct McpStatusRegistry {
    inner: Arc<Mutex<RegistryState>>,
}

impl McpStatusRegistry {
    /// Fresh empty registry. A fresh daemon therefore projects `{}`.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Register `name` in the disabled state together with its validated
    /// config. Rejections (`Duplicate`, `Overflow`, id validation) leave the
    /// registry unchanged.
    /// Private by necessity: its argument is the module-private
    /// [`StoredLocalConfig`], which is never exported or serialized. The
    /// only caller is this module's `POST /api/mcp` handler.
    async fn add(&self, name: &str, config: StoredLocalConfig) -> Result<(), LifecycleError> {
        let mut guard = self.inner.lock().await;
        guard.lifecycle.add(name)?;
        guard.configs.insert(name.to_owned(), config);
        Ok(())
    }

    /// Disable one entry (the only disconnect owned by this package).
    /// Unknown or invalid ids return [`LifecycleError::Unknown`] unchanged.
    pub async fn disable(&self, name: &str) -> Result<(), LifecycleError> {
        let mut guard = self.inner.lock().await;
        guard.lifecycle.set_enabled(name, false)?;
        Ok(())
    }

    /// Whether `name` is a registered entry (id bounds enforced by the
    /// lifecycle registry itself).
    pub async fn contains(&self, name: &str) -> bool {
        let guard = self.inner.lock().await;
        guard.lifecycle.get(name).is_some()
    }

    /// Bounded, deterministic (id-sorted) name-keyed status projection.
    /// Configs, env, commands, tokens and raw errors are never included.
    pub async fn status_map(&self) -> BTreeMap<String, Value> {
        let guard = self.inner.lock().await;
        guard
            .lifecycle
            .list()
            .into_iter()
            .map(|entry| (entry.id, project(&entry.state)))
            .collect()
    }
}

/// Map one lifecycle state to the upstream typed status union.
///
/// `connected` is intentionally absent: it is reported only for a validated
/// live transport handle, and no such handle source exists in this package.
/// A `Ready` helper flag alone is never a connection.
fn project(state: &LifecycleState) -> Value {
    match state {
        LifecycleState::Disabled => json!({"status": "disabled"}),
        LifecycleState::Error { code } => json!({"status": "failed", "error": safe_code(code)}),
        LifecycleState::Enabled | LifecycleState::Starting | LifecycleState::Ready => {
            json!({"status": "failed", "error": NOT_CONNECTED_CODE})
        }
    }
}

/// Echo a stored lifecycle code only when it is a bounded stable code;
/// anything else degrades to [`INTERNAL_CODE`]. Never returns verbatim
/// transport output, commands, env, or credential material.
fn safe_code(code: &str) -> String {
    let stable = !code.is_empty()
        && code.len() <= MAX_ERROR_CODE_LEN
        && code.bytes().all(|b| {
            b.is_ascii_lowercase() || b.is_ascii_digit() || matches!(b, b'_' | b'-' | b'.')
        });
    if stable {
        code.to_owned()
    } else {
        INTERNAL_CODE.to_owned()
    }
}

/// Server-name validation at the trust boundary: non-empty, within the
/// shared id bound, and free of control characters.
fn validate_name(raw: &str) -> Result<&str, ApiFailure> {
    if raw.is_empty() {
        return Err(ApiFailure {
            status: StatusCode::BAD_REQUEST,
            code: "mcp_name_invalid",
            message: "name: required non-empty string".to_owned(),
        });
    }
    if raw.len() > MAX_ID_LEN {
        return Err(ApiFailure {
            status: StatusCode::BAD_REQUEST,
            code: "mcp_name_invalid",
            message: "name: exceeds maximum length".to_owned(),
        });
    }
    if raw.chars().any(char::is_control) {
        return Err(ApiFailure {
            status: StatusCode::BAD_REQUEST,
            code: "mcp_name_invalid",
            message: "name: control characters are not allowed".to_owned(),
        });
    }
    Ok(raw)
}

/// Config-level rejection: stable, secret-free 400. Never echoes the raw
/// config, env values, command line or unknown field contents.
fn config_invalid(message: &str) -> ApiFailure {
    ApiFailure {
        status: StatusCode::BAD_REQUEST,
        code: "mcp_config_invalid",
        message: message.to_owned(),
    }
}

/// Upstream-valid config this build cannot honour: stable 501, no
/// registration. `detail` is one of the `UNSUPPORTED_*` statics above, so
/// no request data can reach the response body.
fn config_unsupported(detail: &'static str) -> ApiFailure {
    ApiFailure {
        status: StatusCode::NOT_IMPLEMENTED,
        code: "mcp_config_unsupported",
        message: format!("config: {detail}"),
    }
}

/// Verdict of the strict add boundary for one raw `config` value.
enum ConfigVerdict {
    /// Explicitly disabled local config, converted to private daemon
    /// storage.
    DisabledLocal(StoredLocalConfig),
    /// Upstream-valid but unsupported here → 501, never registered.
    Unsupported(&'static str),
}

/// Strict boundary validator for one add `config`.
///
/// Accepts **only** `{"type":"local","command":[exe,args...],
/// "enabled":false,"environment"?:{str:str}}` (fields exactly as pinned
/// upstream `config/mcp.ts` `Local`, minus the not-supported optionals),
/// bounded by [`CONFIG_BOUNDS`], and converts it to the internal
/// [`StoredLocalConfig`]. Unknown fields, malformed shapes, missing/typed-wrong
/// `type`, and a non-boolean `enabled` are 400; `type:"remote"`, `enabled`
/// true and omitted `enabled` are 501. Pure and side-effect free: any error
/// leaves the registry untouched.
fn classify_config(raw: &Value) -> Result<ConfigVerdict, ApiFailure> {
    let config = raw
        .as_object()
        .ok_or_else(|| config_invalid("config: must be a JSON object"))?;
    // Unknown payload/config fields first: credentials or OAuth material
    // must be rejected explicitly, never silently dropped (including on a
    // remote config that would otherwise be 501).
    if config
        .keys()
        .any(|key| !ALLOWED_CONFIG_FIELDS.contains(&key.as_str()))
    {
        return Err(config_invalid(
            "config: unknown field; allowed fields are type, command, environment, enabled",
        ));
    }
    match config.get("type").and_then(Value::as_str) {
        Some("local") => {}
        Some("remote") => return Ok(ConfigVerdict::Unsupported(UNSUPPORTED_REMOTE)),
        _ => {
            return Err(config_invalid(
                "config.type: required and must be the string \"local\"",
            ))
        }
    }
    match config.get("enabled") {
        Some(Value::Bool(false)) => {}
        Some(Value::Bool(true)) => return Ok(ConfigVerdict::Unsupported(UNSUPPORTED_ENABLED_TRUE)),
        Some(_) => return Err(config_invalid("config.enabled: must be a boolean")),
        None => return Ok(ConfigVerdict::Unsupported(UNSUPPORTED_ENABLED_DEFAULT)),
    }
    let argv = config
        .get("command")
        .and_then(Value::as_array)
        .filter(|parts| !parts.is_empty())
        .ok_or_else(|| config_invalid("config.command: required non-empty array of strings"))?;
    // Executable + args share the internal arg budget (`max_args` covers
    // the argument tail, so the whole array may hold one more element).
    if argv.len() > CONFIG_BOUNDS.max_args + 1 {
        return Err(config_invalid("config.command: too many elements"));
    }
    let mut parts = argv.iter();
    let executable = parts
        .next()
        .and_then(Value::as_str)
        .filter(|exe| !exe.is_empty())
        .ok_or_else(|| {
            config_invalid("config.command: first element must be a non-empty executable")
        })?;
    let mut args: Vec<String> = Vec::with_capacity(argv.len() - 1);
    for part in parts {
        let part = part
            .as_str()
            .ok_or_else(|| config_invalid("config.command: elements must be strings"))?;
        args.push(part.to_owned());
    }
    if args.iter().map(String::len).sum::<usize>() > CONFIG_BOUNDS.max_args_bytes {
        return Err(config_invalid(
            "config.command: arguments exceed byte budget",
        ));
    }
    let mut env: HashMap<String, String> = HashMap::new();
    if let Some(raw_env) = config.get("environment") {
        let entries = raw_env.as_object().ok_or_else(|| {
            config_invalid("config.environment: must be an object of string values")
        })?;
        if entries.len() > CONFIG_BOUNDS.max_env {
            return Err(config_invalid("config.environment: too many entries"));
        }
        let mut env_bytes = 0usize;
        for (key, value) in entries {
            let value = value
                .as_str()
                .ok_or_else(|| config_invalid("config.environment: values must be strings"))?;
            env_bytes += key.len() + value.len();
            env.insert(key.to_owned(), value.to_owned());
        }
        if env_bytes > CONFIG_BOUNDS.max_env_bytes {
            return Err(config_invalid("config.environment: exceeds byte budget"));
        }
    }
    // Remote/`url` transport is not part of the accepted subset.
    Ok(ConfigVerdict::DisabledLocal(StoredLocalConfig {
        command: executable.to_owned(),
        args,
        env,
    }))
}

fn not_found() -> ApiFailure {
    ApiFailure {
        status: StatusCode::NOT_FOUND,
        code: "mcp_server_not_found",
        message: "mcp server not found".to_owned(),
    }
}

/// `GET /api/mcp` — bounded name-keyed status map (`{}` when fresh).
/// Read-only: spawns nothing, opens no connection, reads no config or
/// credential, mutates nothing.
pub(crate) async fn status(
    Extension(registry): Extension<McpStatusRegistry>,
) -> Json<BTreeMap<String, Value>> {
    Json(registry.status_map().await)
}

/// `POST /api/mcp` — upstream-shaped add: payload `{name, config}`.
///
/// Boundary contract (strict, no silent acceptance):
/// - payload must be a JSON object with exactly `{name, config}`;
///   unknown payload fields → 400 `mcp_add_field_unsupported`
/// - `config` must be an explicitly disabled local config
///   (`type:"local"`, `command:[exe,args...]`, `enabled:false`, optional
///   bounded `environment` with no other fields) — see [`classify_config`];
///   malformed or unknown config fields → 400 with **no side effects**
/// - `enabled` true/omitted and `type:"remote"` → 501
///   `mcp_config_unsupported`, **registered nowhere**
/// - accepted config is registered **disabled** (truthful: the caller asked
///   for disabled), held in daemon memory, and the full status map returns
/// - duplicate name → 409, registry full (64) → 507
///
/// Config detail is never echoed (it may contain secrets); only the stable
/// codes above leave this handler. Nothing here spawns, opens a socket,
/// reads a credential or writes a user file.
pub(crate) async fn add(
    Extension(registry): Extension<McpStatusRegistry>,
    body: Result<Json<Value>, JsonRejection>,
) -> Result<Json<BTreeMap<String, Value>>, ApiFailure> {
    let payload = body
        .map_err(|_| ApiFailure {
            status: StatusCode::BAD_REQUEST,
            code: "mcp_add_malformed",
            message: "body: malformed JSON".to_owned(),
        })?
        .0;
    let payload = payload.as_object().ok_or_else(|| ApiFailure {
        status: StatusCode::BAD_REQUEST,
        code: "mcp_add_malformed",
        message: "body: must be a JSON object".to_owned(),
    })?;
    if payload
        .keys()
        .any(|key| !ALLOWED_PAYLOAD_FIELDS.contains(&key.as_str()))
    {
        return Err(ApiFailure {
            status: StatusCode::BAD_REQUEST,
            code: "mcp_add_field_unsupported",
            message: "body: unknown field; allowed fields are name, config".to_owned(),
        });
    }
    let name = payload
        .get("name")
        .and_then(Value::as_str)
        .ok_or_else(|| ApiFailure {
            status: StatusCode::BAD_REQUEST,
            code: "mcp_add_malformed",
            message: "name: required non-empty string".to_owned(),
        })?;
    let name = validate_name(name)?;
    let raw_config = payload.get("config").ok_or_else(|| ApiFailure {
        status: StatusCode::BAD_REQUEST,
        code: "mcp_add_malformed",
        message: "config: required".to_owned(),
    })?;
    // Validate the whole boundary before touching the registry: any error
    // above or below leaves the registry byte-for-byte unchanged.
    let config = match classify_config(raw_config)? {
        ConfigVerdict::DisabledLocal(config) => config,
        ConfigVerdict::Unsupported(detail) => return Err(config_unsupported(detail)),
    };
    registry
        .add(name, config)
        .await
        .map_err(|error| match error {
            LifecycleError::Duplicate => ApiFailure {
                status: StatusCode::CONFLICT,
                code: "mcp_duplicate_name",
                message: "mcp server name is already registered".to_owned(),
            },
            LifecycleError::Overflow => ApiFailure {
                status: StatusCode::INSUFFICIENT_STORAGE,
                code: "mcp_registry_full",
                message: "mcp registry is full".to_owned(),
            },
            _ => ApiFailure {
                status: StatusCode::BAD_REQUEST,
                code: "mcp_name_invalid",
                message: "name: rejected by mcp registry".to_owned(),
            },
        })?;
    Ok(Json(registry.status_map().await))
}

/// `POST /api/mcp/{name}/disconnect` — genuine disable of a registered
/// entry, acknowledged with upstream's `true`. Unknown name → 404.
pub(crate) async fn disconnect(
    Extension(registry): Extension<McpStatusRegistry>,
    Path(name): Path<String>,
) -> Result<Json<Value>, ApiFailure> {
    let name = validate_name(&name)?;
    registry.disable(name).await.map_err(|error| match error {
        LifecycleError::Unknown => not_found(),
        LifecycleError::IdTooLong | LifecycleError::EmptyId => ApiFailure {
            status: StatusCode::BAD_REQUEST,
            code: "mcp_name_invalid",
            message: "name: invalid".to_owned(),
        },
        other => ApiFailure {
            status: StatusCode::CONFLICT,
            code: "mcp_disconnect_failed",
            message: format!("disconnect rejected: {other}"),
        },
    })?;
    Ok(Json(json!(true)))
}

/// `POST /api/mcp/{name}/connect` — explicit unsupported, never a fake
/// acknowledgement and never a fake `connected` status.
pub(crate) async fn connect(
    Extension(registry): Extension<McpStatusRegistry>,
    Path(name): Path<String>,
) -> Result<Json<Value>, ApiFailure> {
    let name = validate_name(&name)?;
    if !registry.contains(name).await {
        return Err(not_found());
    }
    Err(ApiFailure {
        status: StatusCode::NOT_IMPLEMENTED,
        code: "mcp_connect_unsupported",
        message: "mcp connect is not implemented by this daemon build".to_owned(),
    })
}
