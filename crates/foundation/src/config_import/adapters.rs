//! Pure parse and normalize adapters for imported configuration sources.
//!
//! This module performs no filesystem access, no environment access and never
//! resolves `{env:...}`/`{file:...}` secret references or credential values.
//! Raw credentials on credential-designated fields are replaced with a
//! stable `"[redacted]"` marker; only environment-variable *references* are
//! preserved (`api_key_env`).

use serde_json::{Map, Value};
use std::collections::BTreeMap;

use super::UnsupportedField;

/// Per-source normalized output before cross-source merging.
#[derive(Default, Debug)]
pub(crate) struct SourceOutput {
    pub providers: BTreeMap<String, Value>,
    pub models: BTreeMap<String, Value>,
    pub agents: BTreeMap<String, Value>,
    pub commands: BTreeMap<String, Value>,
    pub base: BTreeMap<String, Value>,
    pub unsupported: Vec<UnsupportedField>,
}

impl SourceOutput {
    #[allow(dead_code)]
    pub(crate) fn is_empty(&self) -> bool {
        self.providers.is_empty()
            && self.models.is_empty()
            && self.agents.is_empty()
            && self.commands.is_empty()
            && self.base.is_empty()
    }
    fn unsupported_at(&mut self, origin: &str, field: &str, reason: &str) {
        self.unsupported.push(UnsupportedField {
            origin: origin.to_string(),
            field: field.to_string(),
            reason: reason.to_string(),
        });
    }
}

/// Outcome of inspecting a credential-designated string value.
#[derive(Debug, PartialEq)]
pub(crate) enum SecretRef {
    /// `{env:VAR}` reference -> environment variable name.
    EnvRef(String),
    /// `{file:...}` reference -> not imported (would need a file read).
    FileRef,
    /// Raw credential literal -> replaced with `"[redacted]"`.
    Redacted,
    /// Any other string on a credential-designated field.
    Plain,
}

#[allow(dead_code)]
const REDACTED: &str = "[redacted]";

pub(crate) fn secret_ref(raw: &str) -> SecretRef {
    let trimmed = raw.trim();
    if let Some(inner) = trimmed.strip_prefix("{env:").and_then(|s| s.strip_suffix('}')) {
        let name = inner.trim();
        if env_name_valid(name) {
            return SecretRef::EnvRef(name.to_string());
        }
        return SecretRef::Redacted; // malformed reference: never keep raw text
    }
    if trimmed.starts_with("{file:") && trimmed.ends_with('}') {
        return SecretRef::FileRef;
    }
    if looks_like_credential(trimmed) {
        return SecretRef::Redacted;
    }
    SecretRef::Plain
}

fn env_name_valid(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 128
        && name
            .chars()
            .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_')
}

fn looks_like_credential(value: &str) -> bool {
    let long_token = value.len() >= 32
        && value
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_');
    value.starts_with("sk-") || long_token
}

/// Remove JSONC comments and trailing commas without touching string contents.
/// Shape proven in `crates/server/src/configured_commands.rs::jsonc`.
pub(crate) fn jsonc(text: &str) -> Result<String, String> {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let (mut index, mut quoted, mut escaped) = (0, false, false);
    while index < bytes.len() {
        let b = bytes[index];
        if quoted {
            out.push(b);
            if escaped {
                escaped = false;
            } else if b == b'\\' {
                escaped = true;
            } else if b == b'"' {
                quoted = false;
            }
        } else if b == b'"' {
            quoted = true;
            out.push(b);
        } else if b == b'/' && bytes.get(index + 1) == Some(&b'/') {
            out.push(b' ');
            index += 2;
            while index < bytes.len() && bytes[index] != b'\n' {
                index += 1;
            }
            continue;
        } else if b == b'/' && bytes.get(index + 1) == Some(&b'*') {
            out.push(b' ');
            index += 2;
            while index + 1 < bytes.len() && &bytes[index..index + 2] != b"*/" {
                index += 1;
            }
            if index + 1 >= bytes.len() {
                return Err("unterminated configuration comment".into());
            }
            index += 2;
            continue;
        } else {
            out.push(b);
        }
        index += 1;
    }
    let mut result = Vec::with_capacity(out.len());
    quoted = false;
    escaped = false;
    for (index, &b) in out.iter().enumerate() {
        if !quoted
            && b == b','
            && out[index + 1..]
                .iter()
                .find(|b| !b.is_ascii_whitespace())
                .is_some_and(|b| matches!(b, b'}' | b']'))
        {
            continue;
        }
        result.push(b);
        if quoted {
            if escaped {
                escaped = false;
            } else if b == b'\\' {
                escaped = true;
            } else if b == b'"' {
                quoted = false;
            }
        } else if b == b'"' {
            quoted = true;
        }
    }
    String::from_utf8(result).map_err(|_| "invalid configuration encoding".to_string())
}

/// Parse a JSON or JSONC configuration document.
pub(crate) fn parse_json_doc(text: &str) -> Result<Value, String> {
    let cleaned = jsonc(text)?;
    serde_json::from_str(&cleaned)
        .map_err(|_| "invalid configuration JSON".to_string())
}

/// Parse a TOML configuration document and convert to JSON for normalization.
pub(crate) fn parse_toml_doc(text: &str) -> Result<Value, String> {
    let parsed: toml::Value = toml::from_str(text).map_err(|error| error.to_string())?;
    serde_json::to_value(parsed).map_err(|_| "unrepresentable TOML value".to_string())
}

/// Agent and command identifiers must match the runtime catalogue rules:
/// no whitespace, no control characters, 1..=128 bytes.
pub(crate) fn identifier_valid(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 128
        && !name.chars().any(|c| c.is_whitespace() || c.is_control())
}

/// Same provider-ID contract enforced by `providers::imported_config`.
pub(crate) fn provider_id_valid(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 128
        && id != "openai"
        && id
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'-' | b'_'))
        && id.as_bytes()[0].is_ascii_alphanumeric()
}

pub(crate) const INVALID_PROVIDER_ID_REASON: &str =
    "provider identifier is not valid for native imported provider consumers";

pub(crate) const UNSUPPORTED_PROVIDER_ID_REASON: &str =
    "provider identifier is reserved or invalid for native imported provider consumers";

pub(crate) fn unsupported_provider_id_reason(id: &str) -> &'static str {
    if id == "openai" {
        UNSUPPORTED_PROVIDER_ID_REASON
    } else {
        INVALID_PROVIDER_ID_REASON
    }
}

const OTHER_SCHEMA: &str =
    "field is outside our supported import schema; it stays in the source file and must be migrated manually";
const NO_EXEC: &str =
    "command/hook/shell configuration is never imported and certainly never executed during discovery";
const NO_AUTHORITY: &str =
    "permission rules and modes are never converted into our permission broker authority; grant access natively instead";

fn as_object<'a>(value: &'a Value) -> Option<&'a Map<String, Value>> {
    value.as_object()
}

/// Normalize one OpenCode configuration document (JSON/JSONC already parsed).
pub(crate) fn normalize_opencode(document: &Value, origin: &str) -> SourceOutput {
    let mut out = SourceOutput::default();
    let Some(document) = as_object(document) else {
        out.unsupported_at(origin, "<document>", "configuration document must be an object");
        return out;
    };
    if let Some(model) = document.get("model").and_then(Value::as_str) {
        out.base.insert("model".into(), Value::String(model.into()));
    }
    for key in ["temperature", "top_p"] {
        if let Some(value) = document.get(key) {
            if value.is_number() {
                out.base.insert(key.into(), value.clone());
            } else {
                out.unsupported_at(origin, key, "expected a number");
            }
        }
    }
    if let Some(agent) = document.get("default_agent").and_then(Value::as_str) {
        out.base
            .insert("default_agent".into(), Value::String(agent.into()));
    }
    // Provider document generations use both "provider" and "providers".
    for key in ["provider", "providers"] {
        if let Some(providers) = document.get(key).and_then(as_object) {
            for (id, entry) in providers {
                normalize_provider(entry, id, origin, &mut out);
            }
        }
    }
    for key in ["agent", "agents"] {
        if let Some(agents) = document.get(key).and_then(as_object) {
            for (name, entry) in agents {
                normalize_agent_map_entry(entry, name, origin, &mut out);
            }
        }
    }
    for key in ["command", "commands"] {
        if let Some(commands) = document.get(key).and_then(as_object) {
            for (name, entry) in commands {
                normalize_command_entry(entry, name, origin, &mut out);
            }
        }
    }
    report_unknown_top_level(document, origin, &mut out, &[
        "model", "temperature", "top_p", "default_agent",
        "provider", "providers", "agent", "agents", "command", "commands",
    ]);
    out
}

/// Normalize one OpenCode provider entry (`provider`/`providers` map value).
///
/// Credential-designated values (`apiKey`) never appear literally in the
/// output: `{env:VAR}` becomes `api_key_env`, anything else is dropped with
/// an explicit unsupported note. `{file:...}` is never read.
pub(crate) fn normalize_provider(entry: &Value, id: &str, origin: &str, out: &mut SourceOutput) {
    if !provider_id_valid(id) {
        out.unsupported_at(
            origin,
            &format!("providers.{id}"),
            unsupported_provider_id_reason(id),
        );
        return;
    }
    let mut provider = Map::new();
    provider.insert("id".into(), Value::String(id.to_string()));
    match entry {
        Value::Object(entry) => {
            if entry.len() > MAX_PROVIDER_FIELDS {
                out.unsupported_at(origin, &format!("providers.{id}"), "provider entry has too many fields; truncated conservatively");
            }
            if let Some(name) = entry.get("name").and_then(Value::as_str) {
                provider.insert("name".into(), Value::String(name.to_string()));
            }
            if entry.get("npm").is_some() {
                out.unsupported_at(origin, &format!("providers.{id}.npm"), "npm package references are outside our import schema; configure the provider natively");
            }
            for container in ["options", "settings"] {
                if let Some(section) = entry.get(container).and_then(Value::as_object) {
                    if let Some(base_url) = section
                        .get("baseURL")
                        .or_else(|| section.get("baseUrl"))
                        .or_else(|| section.get("base_url"))
                        .and_then(Value::as_str)
                    {
                        if provider.get("base_url").is_none() {
                            provider.insert("base_url".into(), Value::String(base_url.to_string()));
                        }
                    }
                    if let Some(api_key) = section.get("apiKey").or_else(|| section.get("api_key")) {
                        apply_credential(&provider_key_path(container), api_key, origin, &format!("providers.{id}"), out, &mut provider);
                    }
                    for key in section.keys() {
                        if !matches!(key.as_str(), "baseURL" | "baseUrl" | "base_url" | "apiKey" | "api_key") {
                            out.unsupported_at(origin, &format!("providers.{id}.{container}.{key}"), "provider transport option is outside our import schema; configure it natively");
                        }
                    }
                }
            }
            if let Some(models) = entry.get("models").and_then(Value::as_object) {
                if models.len() > MAX_MODELS_PER_PROVIDER {
                    out.unsupported_at(origin, &format!("providers.{id}.models"), "too many model entries; import truncated the model id list");
                }
                let mut ids: Vec<String> = Vec::new();
                for (model_id, model_def) in models.iter().take(MAX_MODELS_PER_PROVIDER) {
                    if identifier_valid(model_id) {
                        ids.push(model_id.clone());
                    } else {
                        out.unsupported_at(origin, &format!("providers.{id}.models.{model_id}"), "model identifier contains whitespace or control characters");
                    }
                    if let Some(obj) = model_def.as_object() {
                        for def_key in obj.keys() {
                            if def_key == "id" || def_key == "name" {
                                continue;
                            }
                            out.unsupported_at(
                                origin,
                                &format!("providers.{id}.models.{model_id}.{def_key}"),
                                "model configuration metadata is not imported; only the model identifier is carried",
                            );
                        }
                    }
                }
                ids.sort();
                provider.insert("models".into(), Value::Array(ids.into_iter().map(Value::String).collect()));
            } else if entry.get("models").is_some_and(|v| !v.is_object() && !v.is_null()) {
                out.unsupported_at(origin, &format!("providers.{id}.models"), "expected a model map");
            }
        }
        Value::Null => {}
        _ => out.unsupported_at(origin, &format!("providers.{id}"), "expected a provider object"),
    }
    out.providers.insert(id.to_string(), Value::Object(provider));
}

fn provider_key_path(container: &str) -> String {
    if container == "settings" {
        "settings.apiKey".to_string()
    } else {
        "options.apiKey".to_string()
    }
}

/// Attach a credential-designated field to the provider map with full
/// secret-handling. Raw and `{file:}` values are never carried into the plan.
pub(crate) fn apply_credential(
    label: &str,
    raw: &Value,
    origin: &str,
    field: &str,
    out: &mut SourceOutput,
    provider: &mut Map<String, Value>,
) {
    let Some(text) = raw.as_str() else {
        out.unsupported_at(origin, field, "credential field must be a string");
        return;
    };
    match secret_ref(text) {
        SecretRef::EnvRef(name) => {
            provider.insert("api_key_env".into(), Value::String(name));
        }
        other => {
            let reason = match other {
                SecretRef::FileRef => "file-backed credential references are never read or imported",
                SecretRef::Redacted => "raw credential literal redacted from the import plan; create the credential natively",
                _ => "non-reference credential form is not imported; configure the credential natively",
            };
            out.unsupported_at(origin, &format!("{field}.{label}"), reason);
        }
    }
}

/// Normalize one OpenCode agent map entry (`agent`/`agents` value).
///
/// Accepts the V2 shape (`description`, `mode`, `model`, `system`,
/// expanded model object) and the legacy shape (`prompt`, `temperature`,
/// `top_p`, `tools`, `disable`). Permission rules and modes are reported,
/// never converted into broker authority.
pub(crate) fn normalize_agent_map_entry(
    entry: &Value,
    name: &str,
    origin: &str,
    out: &mut SourceOutput,
) {
    let field_prefix = format!("agents.{name}");
    if !identifier_valid(name) {
        out.unsupported_at(origin, &field_prefix, "agent identifier contains whitespace or control characters");
        return;
    }
    let mut agent = Map::new();
    match entry {
        Value::Object(entry) => {
            if let Some(description) = entry.get("description").and_then(Value::as_str) {
                agent.insert("description".into(), Value::String(description.to_string()));
            }
            if let Some(mode) = entry.get("mode").and_then(Value::as_str) {
                if matches!(mode, "primary" | "subagent" | "all") {
                    agent.insert("mode".into(), Value::String(mode.to_string()));
                } else {
                    out.unsupported_at(origin, &format!("{field_prefix}.mode"), "expected primary, subagent or all; value stays in the source file");
                }
            }
            match entry.get("model") {
                Some(Value::String(model)) => {
                    agent.insert("model".into(), Value::String(model.to_string()));
                }
                Some(Value::Object(model)) => {
                    if let (Some(provider_id), Some(model_id)) = (
                        model.get("providerID").and_then(Value::as_str),
                        model.get("model").and_then(Value::as_str),
                    ) {
                        let variant = model.get("variant").and_then(Value::as_str);
                        let mut composed = format!("{provider_id}/{model_id}");
                        if let Some(variant) = variant {
                            composed.push('#');
                            composed.push_str(variant);
                        }
                        agent.insert("model".into(), Value::String(composed));
                        if model.len() > usize::from(u8::from(variant.is_some())) + 2 {
                            out.unsupported_at(origin, &format!("{field_prefix}.model"), "expanded model object has extra fields; only providerID/model/variant were imported");
                        }
                    } else {
                        out.unsupported_at(origin, &format!("{field_prefix}.model"), "expanded model object needs providerID and model strings");
                    }
                }
                Some(_) => out.unsupported_at(origin, &format!("{field_prefix}.model"), "expected a model string or expanded model object"),
                None => {}
            }
            for prompt_key in ["system", "prompt"] {
                if let Some(prompt) = entry.get(prompt_key).and_then(Value::as_str) {
                    agent.insert("prompt".into(), Value::String(prompt.to_string()));
                    if prompt_key == "prompt" {
                        out.unsupported_at(origin, &format!("{field_prefix}.prompt"), "legacy agent prompt field imported verbatim; new configuration should use `system` or a markdown agent body");
                    }
                } else if entry.get(prompt_key).is_some() {
                    out.unsupported_at(origin, &format!("{field_prefix}.{prompt_key}"), "expected a string prompt");
                }
            }
            for key in ["temperature", "top_p"] {
                match entry.get(key) {
                    Some(Value::Number(number)) => {
                        agent.insert(key.into(), serde_json::Value::Number(number.clone()));
                        if key == "top_p" {
                            out.unsupported_at(origin, &format!("{field_prefix}.{key}"), "legacy sampling field imported verbatim; our runtime cannot represent top_p yet");
                        }
                    }
                    Some(_) => out.unsupported_at(origin, &format!("{field_prefix}.{key}"), "expected a number"),
                    None => {}
                }
            }
            if let Some(tools) = entry.get("tools") {
                if let Some(list) = tools.as_array() {
                    let names: Vec<Value> = list
                        .iter()
                        .filter_map(Value::as_str)
                        .map(|name| Value::String(name.to_string()))
                        .collect();
                    agent.insert("tools".into(), Value::Array(names));
                    out.unsupported_at(origin, &format!("{field_prefix}.tools"), "legacy agent tools field imported verbatim; new configuration should use provider permissions instead");
                } else if tools.as_object().is_some() {
                    out.unsupported_at(origin, &format!("{field_prefix}.tools"), "legacy agent tool map is outside our import schema; configure tool policy natively");
                }
            }
            if entry.get("permissions").is_some() {
                out.unsupported_at(origin, &format!("{field_prefix}.permissions"), NO_AUTHORITY);
            }
            for key in ["disable", "disabled", "hidden"] {
                if entry.get(key).and_then(Value::as_bool) == Some(true) {
                    out.unsupported_at(origin, &format!("{field_prefix}.{key}"), "agent is disabled or hidden upstream; import it only with the flag removed so the definition stays intentional");
                }
            }
        }
        Value::String(prompt) => {
            agent.insert("prompt".into(), Value::String(prompt.to_string()));
        }
        Value::Null => {}
        _ => out.unsupported_at(origin, &field_prefix, "expected an agent object or prompt string"),
    }
    out.agents.insert(name.to_string(), Value::Object(agent));
}

/// Normalize one OpenCode command map entry (`command`/`commands` value).
/// Definitions are stored inertly; nothing is expanded or executed.
pub(crate) fn normalize_command_entry(
    entry: &Value,
    name: &str,
    origin: &str,
    out: &mut SourceOutput,
) {
    let field_prefix = format!("commands.{name}");
    if !identifier_valid(name) {
        out.unsupported_at(origin, &field_prefix, "command identifier contains whitespace or control characters");
        return;
    }
    let mut command = Map::new();
    match entry {
        Value::Object(entry) => {
            match entry.get("template") {
                Some(Value::String(template)) => {
                    command.insert("template".into(), Value::String(template.to_string()));
                }
                Some(_) => out.unsupported_at(origin, &format!("{field_prefix}.template"), "expected a string command template"),
                None => out.unsupported_at(origin, &field_prefix, "command definition has no template"),
            }
            for (key, expected) in [("description", "string"), ("agent", "string"), ("model", "string")] {
                match entry.get(key) {
                    Some(Value::String(value)) => {
                        command.insert(key.into(), Value::String(value.to_string()));
                    }
                    Some(_) => out.unsupported_at(origin, &format!("{field_prefix}.{key}"), &format!("expected a {expected}")),
                    None => {}
                }
            }
            for key in ["subtask", "subagent"] {
                match entry.get(key) {
                    Some(Value::Bool(value)) => {
                        command.insert(key.into(), Value::Bool(*value));
                    }
                    Some(_) => out.unsupported_at(origin, &format!("{field_prefix}.{key}"), "expected a boolean"),
                    None => {}
                }
            }
        }
        Value::String(template) => {
            command.insert("template".into(), Value::String(template.to_string()));
        }
        Value::Null => {}
        _ => out.unsupported_at(origin, &field_prefix, "expected a command object or template string"),
    }
    out.commands.insert(name.to_string(), Value::Object(command));
}

/// Report top-level keys we do not import, categorized. Bounded to
/// [`MAX_UNKNOWN_TOP_LEVEL`] notes so one hostile file cannot flood the plan.
pub(crate) fn unknown_top_level_reason(key: &str) -> Option<&'static str> {
    let key = key.to_ascii_lowercase();
    for family in ["permission", "polic"] {
        if key.contains(family) {
            return Some(NO_AUTHORITY);
        }
    }
    for family in ["hook", "shell", "notify", "mcp", "plugin", "command"] {
        if key.contains(family) {
            return Some(NO_EXEC);
        }
    }
    Some(OTHER_SCHEMA)
}

pub(crate) fn report_unknown_top_level(
    document: &Map<String, Value>,
    origin: &str,
    out: &mut SourceOutput,
    known: &[&str],
) {
    let mut reported = 0usize;
    for key in document.keys() {
        if known.contains(&key.as_str()) {
            continue;
        }
        if reported >= MAX_UNKNOWN_TOP_LEVEL {
            out.unsupported_at(origin, "<top-level>", "more unsupported top-level keys exist; the source file keeps them and they are not imported");
            break;
        }
        let reason = unknown_top_level_reason(key).unwrap_or(OTHER_SCHEMA);
        out.unsupported_at(origin, key, reason);
        reported += 1;
    }
}

pub(crate) const MAX_PROVIDER_FIELDS: usize = 24;
pub(crate) const MAX_MODELS_PER_PROVIDER: usize = 128;
pub(crate) const MAX_UNKNOWN_TOP_LEVEL: usize = 16;
pub(crate) const MAX_PROFILE_NOTES: usize = 8;

/// Parse one Markdown agent document: optional `---` YAML-light frontmatter
/// (`key: value` and `- item` list lines) plus the body. No execution, no
/// network, no file resolution. Body keeps raw bytes of the prompt.
pub(crate) fn parse_markdown_frontmatter(
    raw: &str,
) -> Result<(Vec<(String, String)>, String), String> {
    let mut text = raw;
    if text.len() > MAX_MARKDOWN_BYTES {
        return Err(format!(
            "agent markdown exceeds the maximum size of {MAX_MARKDOWN_BYTES} bytes"
        ));
    }
    if text.contains('\0') {
        return Err("agent markdown contains a NUL byte".to_string());
    }
    let mut lines = text.lines();
    if lines.next().map(str::trim) != Some("---") {
        return Ok((Vec::new(), text.trim_start_matches('\n').to_string()));
    }
    let mut pairs = Vec::new();
    let mut body_starts = None;
    let mut consumed = "---\n".len();
    for line in lines {
        consumed += line.len() + 1;
        if line.trim() == "---" {
            body_starts = Some(consumed);
            break;
        }
        if line.is_empty() {
            continue;
        }
        if let Some(item) = line.trim().strip_prefix("- ") {
            if let Some(last) = pairs.last().map(|(key, _): &(String, String)| key.clone()) {
                pairs.push((last, item.trim().to_string()));
            }
            continue;
        }
        let Some((key, value)) = line.split_once(':') else {
            return Err("agent frontmatter line is not `key: value`; the file is skipped".to_string());
        };
        pairs.push((key.trim().to_string(), value.trim().to_string()));
    }
    let Some(body_starts) = body_starts else {
        return Err("agent frontmatter is not closed with `---`; the file is skipped".to_string());
    };
    text = &text[body_starts.min(text.len())..];
    Ok((pairs, text.trim().to_string()))
}

pub(crate) const MAX_MARKDOWN_BYTES: usize = 64 * 1024;

/// Convert frontmatter pairs (plus markdown body) into a normal agent object.
/// Shared by the OpenCode and Claude agent markdown flavors. Returns the
/// agent name once the entry is stored; `None` means the file was skipped.
pub(crate) fn agent_markdown_entry(
    pairs: &[(String, String)],
    body: &str,
    origin: &str,
    flavor: &str,
    out: &mut SourceOutput,
) -> Option<String> {
    let get = |key: &str| pairs.iter().find(|(k, _)| k == key).map(|(_, v)| v.clone());
    let field = |suffix: &str| format!("{flavor}.{suffix}");
    if let Some(permission_mode) = get("permissionMode") {
        out.unsupported_at(
            origin,
            &field("permissionMode"),
            &format!(
                "permission mode `{permission_mode}` is never converted into our permission broker authority"
            ),
        );
    }
    if let Some(_permissions) = get("permissions") {
        out.unsupported_at(
            origin,
            &field("permissions"),
            "permission rules are never converted into our permission broker authority",
        );
    }
    let mut agent = Map::new();
    if let Some(description) = get("description") {
        agent.insert("description".into(), Value::String(description));
    }
    if let Some(model) = get("model") {
        agent.insert("model".into(), Value::String(model));
    }
    if let Some(temperature) = get("temperature").and_then(|v| v.parse::<f64>().ok()) {
        agent.insert("temperature".into(), serde_json::json!(temperature));
    }
    let mut tools: Vec<Value> = Vec::new();
    for (key, value) in pairs {
        if key == "tools" {
            for part in value.split(',') {
                let part = part.trim();
                if !part.is_empty() {
                    tools.push(Value::String(part.to_string()));
                }
            }
        }
    }
    if !tools.is_empty() {
        agent.insert("tools".into(), Value::Array(tools));
    }
    agent.insert("prompt".into(), Value::String(body.to_string()));
    let name = get("name")?;
    out.agents.insert(name.clone(), Value::Object(agent));
    Some(name)
}

/// Qualify a Codex `model` value as `provider/model` when it is unqualified.
/// A model containing `/` is already provider-qualified and is returned as-is.
/// When `model_provider` is present and non-empty, the root model is prefixed;
/// agent-local `model` fields are qualified per the agent's resolved provider.
fn qualify_codex_model(model: &str, document: &serde_json::Map<String, Value>) -> String {
    if model.contains('/') {
        return model.to_string();
    }
    if let Some(provider) = document.get("model_provider").and_then(Value::as_str) {
        if !provider.is_empty() && identifier_valid(provider) {
            return format!("{provider}/{model}");
        }
    }
    model.to_string()
}

/// Normalize one Codex `config.toml` document (TOML already parsed to JSON).
///
/// Imports: `model`, `model_providers.<id>` (`name`, `base_url`, `env_key`),
/// `agents.<name>.description`. Everything else that carries authority,
/// shell execution or credential material is reported and dropped.
pub(crate) fn normalize_codex(document: &Value, origin: &str) -> SourceOutput {
    let mut out = SourceOutput::default();
    let Some(document) = as_object(document) else {
        out.unsupported_at(origin, "<document>", "configuration document must be an object");
        return out;
    };
    if let Some(model) = document.get("model").and_then(Value::as_str) {
        let qualified = qualify_codex_model(model, document);
        out.base.insert("model".into(), Value::String(qualified));
    }
    if let Some(provider) = document.get("model_provider").and_then(Value::as_str) {
        out.unsupported_at(
            origin,
            "model_provider",
            format!("provider selection (`{provider}`) is not converted; import the provider table and choose natively").as_str(),
        );
    }
    if let Some(providers) = document.get("model_providers").and_then(Value::as_object) {
        for (id, entry) in providers {
            normalize_codex_provider(entry, id, origin, &mut out);
        }
    }
    if let Some(profiles) = document.get("profiles").and_then(Value::as_object) {
        for (name, _) in profiles.iter().take(MAX_PROFILE_NOTES) {
            out.unsupported_at(
                origin,
                format!("profiles.{name}").as_str(),
                "named profiles are not imported; starting a profile is a runtime choice, not a setting value",
            );
        }
    }
    if let Some(agents) = document.get("agents").and_then(Value::as_object) {
        for (name, entry) in agents {
            normalize_codex_agent(entry, name, origin, &mut out);
        }
    }
    report_unknown_top_level(document, origin, &mut out, &[
        "model", "model_provider", "model_providers", "profiles", "agents",
    ]);
    out
}

fn normalize_codex_provider(entry: &Value, id: &str, origin: &str, out: &mut SourceOutput) {
    let field_prefix = format!("model_providers.{id}");
    if !provider_id_valid(id) {
        out.unsupported_at(origin, &field_prefix, unsupported_provider_id_reason(id));
        return;
    }
    let mut provider = Map::new();
    provider.insert("id".into(), Value::String(id.to_string()));
    match entry {
        Value::Object(entry) => {
            if let Some(name) = entry.get("name").and_then(Value::as_str) {
                provider.insert("name".into(), Value::String(name.into()));
            }
            if let Some(base_url) = entry.get("base_url").and_then(Value::as_str) {
                provider.insert("base_url".into(), Value::String(base_url.into()));
            }
            if let Some(env_key) = entry.get("env_key").and_then(Value::as_str) {
                if env_name_valid(env_key) {
                    provider.insert("api_key_env".into(), Value::String(env_key.into()));
                } else {
                    out.unsupported_at(origin, &format!("{field_prefix}.env_key"), "environment variable name is not a valid reference; configure the credential natively");
                }
            }
            if entry.get("experimental_bearer_token").is_some()
                || entry.get("auth").is_some()
            {
                out.unsupported_at(
                    origin,
                    &field_prefix,
                    "command-backed or inline bearer-token credential material is never imported; create the credential natively",
                );
            }
            if let Some(wire_api) = entry.get("wire_api").and_then(Value::as_str) {
                if wire_api != "responses" {
                    out.unsupported_at(origin, &format!("{field_prefix}.wire_api"), &format!("wire protocol `{wire_api}` is outside the documented configuration reference; only `responses` is imported with confidence"));
                }
            }
            for key in entry.keys() {
                if !matches!(key.as_str(), "name" | "base_url" | "env_key" | "wire_api" | "auth" | "experimental_bearer_token") {
                    out.unsupported_at(origin, &format!("{field_prefix}.{key}"), "provider option is outside our import schema; configure it natively");
                }
            }
        }
        _ => out.unsupported_at(origin, &field_prefix, "expected a provider table"),
    }
    let has_base = provider.contains_key("base_url");
    let has_key = provider.contains_key("api_key_env");
    out.providers.insert(id.to_string(), Value::Object(provider));
    if !has_base && !has_key {
        // Still recorded: an id-only alias may resolve natively. The absence is
        // surfaced as provenance instead of an error because the reference
        // documents `model_providers.<id>` tables with partial fields.
        out.unsupported_at(origin, &field_prefix, "provider table has no base_url or env_key; nothing concrete to import");
    }
}

fn normalize_codex_agent(entry: &Value, name: &str, origin: &str, out: &mut SourceOutput) {
    let field_prefix = format!("agents.{name}");
    match entry {
        Value::Object(entry) => {
            if identifier_valid(name) {
                let mut agent = Map::new();
                if let Some(description) = entry.get("description").and_then(Value::as_str) {
                    agent.insert("description".into(), Value::String(description.into()));
                }
                agent.insert("mode".into(), Value::String("subagent".into()));
                if entry.get("config_file").is_some() {
                    out.unsupported_at(
                        origin,
                        &format!("{field_prefix}.config_file"),
                        "agent config_file layers are not imported; read the referenced file explicitly if you want it",
                    );
                }
                out.agents.insert(name.to_string(), Value::Object(agent));
            } else {
                out.unsupported_at(origin, &field_prefix, "agent role identifier contains whitespace or control characters");
            }
            for key in entry.keys() {
                if !matches!(key.as_str(), "description" | "config_file") {
                    out.unsupported_at(origin, &format!("{field_prefix}.{key}"), "agent setting is outside our import schema; configure it natively");
                }
            }
        }
        _ => out.unsupported_at(origin, &field_prefix, "expected an agent settings table; scalar multi-agent settings are not imported"),
    }
}

/// Normalize one Claude Code settings document (strict JSON, already parsed).
///
/// Imports only `model`. The `env` block and every other key (`permissions`,
/// `hooks`, `mcpServers`, `apiKeyHelper`, ...) are reported as not imported:
/// they either execute, grant authority or need runtime representation we do
/// not have. Claude Code settings are strict JSON, so parsing stays strict.
pub(crate) fn normalize_claude(document: &Value, origin: &str) -> SourceOutput {
    let mut out = SourceOutput::default();
    let Some(document) = as_object(document) else {
        out.unsupported_at(origin, "<document>", "configuration document must be an object");
        return out;
    };
    if let Some(model) = document.get("model").and_then(Value::as_str) {
        out.base.insert("model".into(), Value::String(model.into()));
    } else if document.get("model").is_some() {
        out.unsupported_at(origin, "model", "expected a string model value");
    }
    if let Some(env) = document.get("env").and_then(Value::as_object) {
        if env.is_empty() {
            out.unsupported_at(origin, "env", "empty env block; nothing to import even natively");
        } else {
            let names: Vec<&str> = env.keys().take(8).map(String::as_str).collect();
            out.unsupported_at(
                origin,
                "env",
                format!("environment variables are never imported ({}) ; configure runtime environment natively", names.join(", ")).as_str(),
            );
        }
    }
    report_unknown_top_level(document, origin, &mut out, &["model", "env"]);
    out
}

