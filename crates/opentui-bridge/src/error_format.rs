#![forbid(unsafe_code)]
//! Error formatting parity for `packages/tui/src/util/error.ts`.
//!
//! - `:19-75`: Provider, Config, cancellation, and MCP message mapping.
//! - `:97-123`: native, record, and scalar formatting.
//! - `:125-145`: message extraction plus cause-chain joining.
//! - `:147-181`: native/record data projection and complex-value debug strings.
//!
//! Native errors and tagged records share one owned value tree. This avoids a
//! JavaScript runtime while preserving message precedence, object inspection,
//! caller-owned exit codes, and bounded-safe formatting. `lib.rs` registration
//! remains integration-owned.

use std::fmt;

/// Maximum nesting rendered by the pretty object formatter.
pub const MAX_JSON_DEPTH: usize = 64;

/// Native error fields used by JavaScript `Error` instances.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeError {
    pub name: String,
    pub message: String,
    pub stack: Option<String>,
    pub cause: Option<Box<Value>>,
}

impl NativeError {
    #[must_use]
    pub fn new(name: &str, message: &str) -> Self {
        Self {
            name: name.to_string(),
            message: message.to_string(),
            stack: None,
            cause: None,
        }
    }

    #[must_use]
    pub fn with_stack(mut self, stack: &str) -> Self {
        self.stack = Some(stack.to_string());
        self
    }

    #[must_use]
    pub fn with_cause(mut self, cause: Value) -> Self {
        self.cause = Some(Box::new(cause));
        self
    }
}

/// Ordered record fields. Order is retained for JSON formatting parity.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Object {
    constructor: String,
    fields: Vec<(String, Value)>,
}

impl Object {
    #[must_use]
    pub fn new() -> Self {
        Self {
            constructor: "Object".to_string(),
            fields: Vec::new(),
        }
    }

    #[must_use]
    pub fn tagged(tag: &str) -> Self {
        Self::new().with("_tag", Value::string(tag))
    }

    #[must_use]
    pub fn with(mut self, key: &str, value: Value) -> Self {
        if let Some((_, current)) = self.fields.iter_mut().find(|(name, _)| name == key) {
            *current = value;
        } else {
            self.fields.push((key.to_string(), value));
        }
        self
    }

    #[must_use]
    pub fn get(&self, key: &str) -> Option<&Value> {
        self.fields
            .iter()
            .find(|(name, _)| name == key)
            .map(|(_, value)| value)
    }

    pub fn iter(&self) -> impl Iterator<Item = (&str, &Value)> {
        self.fields.iter().map(|(key, value)| (key.as_str(), value))
    }

    #[must_use]
    pub fn constructor(&self) -> &str {
        &self.constructor
    }
}

impl Default for Object {
    fn default() -> Self {
        Self::new()
    }
}

/// Minimal unknown-value model covering every branch used by `error.ts`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Value {
    Error(NativeError),
    Object(Object),
    Array(Vec<Value>),
    String(String),
    Integer(i64),
    Boolean(bool),
    Null,
    Undefined,
}

impl Value {
    #[must_use]
    pub fn string(value: &str) -> Self {
        Self::String(value.to_string())
    }

    #[must_use]
    pub fn tagged(tag: &str, message: &str) -> Self {
        Self::Object(Object::tagged(tag).with("message", Self::string(message)))
    }

    #[must_use]
    pub fn tagged_with_data(tag: &str, message: &str, data: Value) -> Self {
        Self::Object(
            Object::tagged(tag)
                .with("message", Self::string(message))
                .with("data", data),
        )
    }

    #[must_use]
    pub fn none() -> Self {
        Self::Undefined
    }
}

/// Property value returned by [`error_data`]. Complex values use a debug string.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DataValue {
    String(String),
    Integer(i64),
    Boolean(bool),
    Text(String),
    Debug(String),
}

impl DataValue {
    #[must_use]
    pub fn as_string(&self) -> String {
        match self {
            Self::String(value) | Self::Text(value) | Self::Debug(value) => value.clone(),
            Self::Integer(value) => value.to_string(),
            Self::Boolean(value) => value.to_string(),
        }
    }
}

impl fmt::Display for DataValue {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.as_string())
    }
}

/// Stable error-data projection used by the native UI.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ErrorData {
    pub type_name: String,
    pub message: String,
    pub stack: Option<String>,
    pub cause: Option<String>,
    pub formatted: String,
    pub properties: Vec<(String, DataValue)>,
    pub data: Option<String>,
}

/// Message and exit code returned without a process side effect.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CliFailure {
    pub message: String,
    pub exit_code: Option<i32>,
}

fn as_object(value: &Value) -> Option<&Object> {
    match value {
        Value::Object(object) => Some(object),
        _ => None,
    }
}

fn string_field<'a>(object: &'a Object, key: &str) -> Option<&'a str> {
    match object.get(key) {
        Some(Value::String(value)) => Some(value),
        _ => None,
    }
}

fn field_or_undefined(object: &Object, key: &str) -> String {
    string_field(object, key).unwrap_or("undefined").to_string()
}

fn native_to_string(error: &NativeError) -> String {
    if error.message.is_empty() {
        if error.name.is_empty() {
            "Error".to_string()
        } else {
            error.name.clone()
        }
    } else if error.name.is_empty() {
        error.message.clone()
    } else {
        format!("{}: {}", error.name, error.message)
    }
}

fn array_to_string(values: &[Value]) -> String {
    values
        .iter()
        .map(|value| match value {
            Value::Error(error) => native_to_string(error),
            Value::Object(_) => "[object Object]".to_string(),
            Value::Array(_) => "[object Array]".to_string(),
            Value::String(value) => value.clone(),
            Value::Integer(value) => value.to_string(),
            Value::Boolean(value) => value.to_string(),
            Value::Null | Value::Undefined => String::new(),
        })
        .collect::<Vec<_>>()
        .join(",")
}

fn string_fallback(value: &Value) -> String {
    match value {
        Value::Error(error) => native_to_string(error),
        Value::Object(_) => "[object Object]".to_string(),
        Value::Array(values) => array_to_string(values),
        Value::String(value) => value.clone(),
        Value::Integer(value) => value.to_string(),
        Value::Boolean(value) => value.to_string(),
        Value::Null => "null".to_string(),
        Value::Undefined => "undefined".to_string(),
    }
}

fn base_message(value: &Value) -> String {
    match value {
        Value::Error(error) => {
            if !error.message.is_empty() {
                error.message.clone()
            } else if !error.name.is_empty() {
                error.name.clone()
            } else {
                "Error".to_string()
            }
        }
        Value::Object(object) => {
            if let Some(message) = string_field(object, "message") {
                if !message.is_empty() {
                    return message.to_string();
                }
            }
            if let Some(Value::Object(data)) = object.get("data") {
                if let Some(message) = string_field(data, "message") {
                    if !message.is_empty() {
                        return message.to_string();
                    }
                }
            }
            error_format(value)
        }
        Value::Array(values) => {
            let text = array_to_string(values);
            if text.is_empty() || text == "[object Object]" {
                error_format(value)
            } else {
                text
            }
        }
        _ => string_fallback(value),
    }
}

fn next_native_cause(value: &Value) -> Option<&Value> {
    match value {
        Value::Error(error) => error.cause.as_deref(),
        _ => None,
    }
}

fn json_quote(value: &str) -> String {
    let mut output = String::with_capacity(value.len() + 2);
    output.push('"');
    for character in value.chars() {
        match character {
            '"' => output.push_str("\\\""),
            '\\' => output.push_str("\\\\"),
            '\u{08}' => output.push_str("\\b"),
            '\u{0c}' => output.push_str("\\f"),
            '\n' => output.push_str("\\n"),
            '\r' => output.push_str("\\r"),
            '\t' => output.push_str("\\t"),
            character if character <= '\u{1f}' => {
                use std::fmt::Write;
                let _ = write!(output, "\\u{:04x}", character as u32);
            }
            character => output.push(character),
        }
    }
    output.push('"');
    output
}

fn json_value(value: &Value, depth: usize) -> Option<String> {
    if depth > MAX_JSON_DEPTH {
        return None;
    }
    match value {
        Value::Error(_) => Some("{}".to_string()),
        Value::Object(object) => {
            let mut entries = Vec::new();
            for (key, field) in object.iter() {
                if matches!(field, Value::Undefined) {
                    continue;
                }
                if let Some(field) = json_value(field, depth + 1) {
                    entries.push(format!("{}: {field}", json_quote(key)));
                } else {
                    return None;
                }
            }
            if entries.is_empty() {
                return Some("{}".to_string());
            }
            let inner = "  ".repeat(depth + 1);
            let outer = "  ".repeat(depth);
            Some(format!(
                "{{\n{inner}{}\n{outer}}}",
                entries.join(&format!(",\n{inner}"))
            ))
        }
        Value::Array(values) => {
            if values.is_empty() {
                return Some("[]".to_string());
            }
            let inner = "  ".repeat(depth + 1);
            let outer = "  ".repeat(depth);
            let mut entries = Vec::with_capacity(values.len());
            for value in values {
                let entry = if matches!(value, Value::Undefined) {
                    "null".to_string()
                } else {
                    json_value(value, depth + 1)?
                };
                entries.push(format!("{inner}{entry}"));
            }
            Some(format!("[\n{}\n{outer}]", entries.join(",\n")))
        }
        Value::String(value) => Some(json_quote(value)),
        Value::Integer(value) => Some(value.to_string()),
        Value::Boolean(value) => Some(value.to_string()),
        Value::Null => Some("null".to_string()),
        Value::Undefined => Some("undefined".to_string()),
    }
}

/// Format native errors, records, or any non-error unknown.
#[must_use]
pub fn error_format(error: &Value) -> String {
    match error {
        Value::Error(native) => {
            if let Some(stack) = &native.stack {
                return stack.clone();
            }
            let name = if native.name.is_empty() {
                "Error"
            } else {
                native.name.as_str()
            };
            format!("{name}: {}", error_message(error))
        }
        Value::Object(_) | Value::Array(_) => {
            json_value(error, 0).unwrap_or_else(|| "Unexpected error (unserializable)".to_string())
        }
        _ => string_fallback(error),
    }
}

/// Extract a non-empty message, following native cause chains.
#[must_use]
pub fn error_message(error: &Value) -> String {
    let mut message = base_message(error);
    let mut cause = next_native_cause(error);
    while let Some(value) = cause {
        let cause_message = base_message(value);
        if !cause_message.is_empty() {
            if !message.is_empty() {
                message.push_str(": cause: ");
            }
            message.push_str(&cause_message);
        }
        cause = next_native_cause(value);
    }
    if message.is_empty() {
        "unknown error".to_string()
    } else {
        message
    }
}

fn set_property(properties: &mut Vec<(String, DataValue)>, key: &str, value: DataValue) {
    if let Some((_, current)) = properties.iter_mut().find(|(name, _)| name == key) {
        *current = value;
    } else {
        properties.push((key.to_string(), value));
    }
}

fn data_value(value: &Value) -> Option<DataValue> {
    match value {
        Value::String(value) => Some(DataValue::String(value.clone())),
        Value::Integer(value) => Some(DataValue::Integer(*value)),
        Value::Boolean(value) => Some(DataValue::Boolean(*value)),
        Value::Error(value) => Some(DataValue::Text(value.message.clone())),
        Value::Object(_) | Value::Array(_) => Some(DataValue::Debug(format!("{value:?}"))),
        Value::Null => Some(DataValue::Text("null".to_string())),
        Value::Undefined => None,
    }
}

fn non_record_type(error: &Value) -> &'static str {
    match error {
        Value::String(_) => "string",
        Value::Integer(_) => "number",
        Value::Boolean(_) => "boolean",
        Value::Array(_) | Value::Null => "object",
        Value::Undefined => "undefined",
        Value::Error(_) | Value::Object(_) => "object",
    }
}

/// Project native and record errors into stable display data.
#[must_use]
pub fn error_data(error: &Value) -> ErrorData {
    match error {
        Value::Error(native) => ErrorData {
            type_name: native.name.clone(),
            message: error_message(error),
            stack: native.stack.clone(),
            cause: native.cause.as_deref().map(error_format),
            formatted: error_format(error),
            properties: Vec::new(),
            data: None,
        },
        Value::Object(object) => {
            let mut properties = Vec::new();
            for (key, value) in object.iter() {
                if let Some(value) = data_value(value) {
                    properties.push((key.to_string(), value));
                }
            }
            let message = properties
                .iter()
                .find(|(key, _)| key == "message")
                .map_or_else(|| error_message(error), |(_, value)| value.as_string());
            if !properties
                .iter()
                .any(|(key, value)| key == "message" && matches!(value, DataValue::String(_)))
            {
                set_property(
                    &mut properties,
                    "message",
                    DataValue::String(message.clone()),
                );
            }
            set_property(
                &mut properties,
                "formatted",
                DataValue::String(error_format(error)),
            );
            let type_name = match properties
                .iter()
                .find(|(key, _)| key == "type")
                .map(|(_, value)| value)
            {
                Some(DataValue::String(value)) => value.clone(),
                _ => object.constructor().to_string(),
            };
            let data = properties
                .iter()
                .find(|(key, _)| key == "data")
                .map(|(_, value)| value.as_string());
            ErrorData {
                type_name,
                message,
                stack: None,
                cause: None,
                formatted: error_format(error),
                properties,
                data,
            }
        }
        _ => ErrorData {
            type_name: non_record_type(error).to_string(),
            message: error_message(error),
            stack: None,
            cause: None,
            formatted: error_format(error),
            properties: Vec::new(),
            data: None,
        },
    }
}

fn config_data<'a>(error: &'a Value, tag: &str) -> Option<&'a Object> {
    let object = as_object(error)?;
    if string_field(object, "name") == Some(tag) {
        if let Some(Value::Object(data)) = object.get("data") {
            return Some(data);
        }
    }
    if string_field(object, "_tag") == Some(tag) {
        return Some(object);
    }
    None
}

fn string_array(object: &Object, key: &str) -> Vec<String> {
    match object.get(key) {
        Some(Value::Array(values)) => values
            .iter()
            .filter_map(|value| match value {
                Value::String(value) => Some(value.clone()),
                _ => None,
            })
            .collect(),
        _ => Vec::new(),
    }
}

fn config_issue_line(value: &Value) -> Option<String> {
    let object = as_object(value)?;
    let message = string_field(object, "message")?;
    let path = match object.get("path") {
        Some(Value::Array(path)) => path,
        _ => return None,
    };
    let mut parts = Vec::with_capacity(path.len());
    for value in path {
        match value {
            Value::String(value) => parts.push(value.as_str()),
            _ => return None,
        }
    }
    Some(format!("\u{21b3} {message} {}", parts.join(".")))
}

fn known_cli_message(error: &Value) -> Option<String> {
    let object = as_object(error)?;

    if string_field(object, "_tag") == Some("CliError") {
        return Some(string_field(object, "message").unwrap_or("").to_string());
    }
    if string_field(object, "_tag") == Some("AccountServiceError")
        || string_field(object, "_tag") == Some("AccountTransportError")
    {
        return Some(string_field(object, "message").unwrap_or("").to_string());
    }

    if let Some(model) = config_data(error, "ProviderModelNotFoundError") {
        let mut lines = vec![format!(
            "Model not found: {}/{}",
            field_or_undefined(model, "providerID"),
            field_or_undefined(model, "modelID")
        )];
        let suggestions = string_array(model, "suggestions");
        if !suggestions.is_empty() {
            lines.push(format!("Did you mean: {}", suggestions.join(", ")));
        }
        lines.push("Try: `opencode models` to list available models".to_string());
        lines.push("Or check your config (opencode.json) provider/model names".to_string());
        return Some(lines.join("\n"));
    }

    if let Some(provider) = config_data(error, "ProviderInitError") {
        return Some(format!(
            "Failed to initialize provider \"{}\". Check credentials and configuration.",
            field_or_undefined(provider, "providerID")
        ));
    }

    if let Some(json) = config_data(error, "ConfigJsonError") {
        let mut message = format!(
            "Config file at {} is not valid JSON(C)",
            field_or_undefined(json, "path")
        );
        if let Some(detail) = string_field(json, "message") {
            if !detail.is_empty() {
                message.push_str(": ");
                message.push_str(detail);
            }
        }
        return Some(message);
    }

    if let Some(directory) = config_data(error, "ConfigDirectoryTypoError") {
        return Some(format!(
            "Directory \"{}\" in {} is not valid. Rename the directory to \"{}\" or remove it. This is a common typo.",
            field_or_undefined(directory, "dir"),
            field_or_undefined(directory, "path"),
            field_or_undefined(directory, "suggestion")
        ));
    }

    if let Some(frontmatter) = config_data(error, "ConfigFrontmatterError") {
        return Some(
            string_field(frontmatter, "message")
                .unwrap_or("")
                .to_string(),
        );
    }

    if let Some(invalid) = config_data(error, "ConfigInvalidError") {
        let path = string_field(invalid, "path").unwrap_or("");
        let mut first = if path.is_empty() || path == "config" {
            "Configuration is invalid".to_string()
        } else {
            format!("Configuration is invalid at {path}")
        };
        if let Some(message) = string_field(invalid, "message") {
            if !message.is_empty() {
                first.push_str(": ");
                first.push_str(message);
            }
        }
        let mut lines = vec![first];
        if let Some(Value::Array(issues)) = invalid.get("issues") {
            for issue in issues {
                if let Some(line) = config_issue_line(issue) {
                    lines.push(line);
                }
            }
        }
        return Some(lines.join("\n"));
    }

    if string_field(object, "_tag") == Some("UICancelledError")
        || string_field(object, "name") == Some("UICancelledError")
    {
        return Some(String::new());
    }

    if string_field(object, "name") == Some("MCPFailed")
        || string_field(object, "_tag") == Some("MCPFailed")
    {
        let name = match object.get("data") {
            Some(Value::Object(data)) => field_or_undefined(data, "name"),
            _ => "undefined".to_string(),
        };
        return Some(format!(
            "MCP server \"{name}\" failed. Note, opencode does not support MCP authentication yet."
        ));
    }

    None
}

fn body_chain<'a>(error: &'a Value) -> Vec<&'a Value> {
    let mut bodies = Vec::new();
    let mut current = error;
    while let Value::Error(native) = current {
        let cause = match native.cause.as_deref() {
            Some(Value::Object(cause)) => cause,
            _ => break,
        };
        let body = match cause.get("body") {
            Some(body) => body,
            None => break,
        };
        bodies.push(body);
        current = body;
    }
    bodies
}

fn mapped_exit_code(error: &Value) -> Option<i32> {
    let object = as_object(error)?;
    if string_field(object, "_tag") != Some("CliError") {
        return None;
    }
    match object.get("exitCode") {
        Some(Value::Integer(value)) if *value >= i32::MIN as i64 && *value <= i32::MAX as i64 => {
            Some(*value as i32)
        }
        _ => None,
    }
}

/// Map Cli, Account, Config, Provider, cancellation, and MCP messages.
#[must_use]
pub fn cli_error_message(error: &Value) -> Option<String> {
    for body in body_chain(error).into_iter().rev() {
        if let Some(message) = known_cli_message(body) {
            if !message.is_empty() {
                return Some(message);
            }
        }
    }
    known_cli_message(error)
}

/// Return the mapped CliError exit code; callers decide whether to apply it.
#[must_use]
pub fn cli_exit_code(error: &Value) -> Option<i32> {
    let mut nested_code = None;
    for body in body_chain(error).into_iter().rev() {
        match known_cli_message(body) {
            Some(message) if !message.is_empty() => return mapped_exit_code(body),
            Some(_) => {
                if let Some(code) = mapped_exit_code(body) {
                    nested_code = Some(code);
                }
            }
            None => {}
        }
    }
    if known_cli_message(error).is_some() {
        mapped_exit_code(error).or(nested_code)
    } else {
        nested_code
    }
}

/// Message plus caller-owned exit code for known CLI failures.
#[must_use]
pub fn cli_failure(error: &Value) -> Option<CliFailure> {
    Some(CliFailure {
        message: cli_error_message(error)?,
        exit_code: cli_exit_code(error),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nested_cause_message_joins_with_cause_prefix() {
        let leaf = Value::Error(NativeError::new("Error", "disk offline"));
        let middle = Value::Error(NativeError::new("Error", "write failed").with_cause(leaf));
        let outer = Value::Error(NativeError::new("Error", "save failed").with_cause(middle));

        assert_eq!(
            error_message(&outer),
            "save failed: cause: write failed: cause: disk offline"
        );
        assert_eq!(
            error_format(&outer),
            "Error: save failed: cause: write failed: cause: disk offline"
        );
    }

    #[test]
    fn tagged_cli_shape_keeps_data_and_returns_exit_code_separately() {
        let data = Value::Object(Object::new().with("requestId", Value::string("req-7")));
        let error = Value::Object(
            Object::tagged("CliError")
                .with("message", Value::string("request rejected"))
                .with("data", data)
                .with("exitCode", Value::Integer(7)),
        );

        assert_eq!(
            cli_failure(&error),
            Some(CliFailure {
                message: "request rejected".to_string(),
                exit_code: Some(7),
            })
        );
        assert_eq!(cli_exit_code(&error), Some(7));
    }

    #[test]
    fn tagged_data_passes_through_as_debug_string() {
        let data = Value::Object(Object::new().with("requestId", Value::string("req-7")));
        let expected = format!("{data:?}");
        let error = Value::tagged_with_data("CliError", "request rejected", data);

        assert_eq!(error_data(&error).data.as_deref(), Some(expected.as_str()));
    }

    #[test]
    fn unknown_integer_uses_string_fallback() {
        let error = Value::Integer(7);

        assert_eq!(error_format(&error), "7");
        assert_eq!(error_message(&error), "7");
        assert_eq!(error_data(&error).type_name, "number");
    }

    #[test]
    fn none_uses_string_fallback() {
        let error = Value::none();

        assert_eq!(error_format(&error), "undefined");
        assert_eq!(error_message(&error), "undefined");
        assert_eq!(error_data(&error).type_name, "undefined");
    }

    #[test]
    fn mcp_named_error_branch_maps_message() {
        let data = Value::Object(Object::new().with("name", Value::string("weather")));
        let error = Value::Object(
            Object::new()
                .with("name", Value::string("MCPFailed"))
                .with("data", data),
        );

        assert_eq!(
            cli_error_message(&error).as_deref(),
            Some("MCP server \"weather\" failed. Note, opencode does not support MCP authentication yet.")
        );
        assert_eq!(cli_exit_code(&error), None);
    }

    #[test]
    fn provider_model_named_error_maps_suggestions() {
        let data = Value::Object(
            Object::new()
                .with("providerID", Value::string("anthropic"))
                .with("modelID", Value::string("sonnet-4"))
                .with(
                    "suggestions",
                    Value::Array(vec![
                        Value::string("sonnet-4.5"),
                        Value::Integer(1),
                        Value::string("sonnet-4-5"),
                    ]),
                ),
        );
        let error = Value::Object(
            Object::new()
                .with("name", Value::string("ProviderModelNotFoundError"))
                .with("data", data),
        );

        assert_eq!(
            cli_error_message(&error).as_deref(),
            Some(
                "Model not found: anthropic/sonnet-4\nDid you mean: sonnet-4.5, sonnet-4-5\nTry: `opencode models` to list available models\nOr check your config (opencode.json) provider/model names"
            )
        );
    }

    #[test]
    fn config_and_provider_branches_map_messages() {
        let provider = Value::Object(
            Object::tagged("ProviderInitError")
                .with("message", Value::string(""))
                .with("providerID", Value::string("openai")),
        );
        assert_eq!(
            cli_error_message(&provider).as_deref(),
            Some("Failed to initialize provider \"openai\". Check credentials and configuration.")
        );

        let json = Value::Object(
            Object::tagged("ConfigJsonError")
                .with("path", Value::string("opencode.json"))
                .with("message", Value::string("bad token")),
        );
        assert_eq!(
            cli_error_message(&json).as_deref(),
            Some("Config file at opencode.json is not valid JSON(C): bad token")
        );

        let directory = Value::Object(
            Object::tagged("ConfigDirectoryTypoError")
                .with("dir", Value::string("models"))
                .with("path", Value::string("provider.json"))
                .with("suggestion", Value::string("model")),
        );
        assert_eq!(
            cli_error_message(&directory).as_deref(),
            Some("Directory \"models\" in provider.json is not valid. Rename the directory to \"model\" or remove it. This is a common typo.")
        );

        let frontmatter = Value::tagged("ConfigFrontmatterError", "invalid frontmatter");
        assert_eq!(
            cli_error_message(&frontmatter).as_deref(),
            Some("invalid frontmatter")
        );

        let issue = Value::Object(
            Object::new()
                .with("message", Value::string("expected string"))
                .with(
                    "path",
                    Value::Array(vec![Value::string("provider"), Value::string("apiKey")]),
                ),
        );
        let invalid = Value::Object(
            Object::tagged("ConfigInvalidError")
                .with("path", Value::string("config"))
                .with("message", Value::string("schema mismatch"))
                .with("issues", Value::Array(vec![issue])),
        );
        assert_eq!(
            cli_error_message(&invalid).as_deref(),
            Some("Configuration is invalid: schema mismatch\n↳ expected string provider.apiKey")
        );
    }

    #[test]
    fn account_and_cancelled_branches_map_messages() {
        for tag in ["AccountServiceError", "AccountTransportError"] {
            assert_eq!(
                cli_error_message(&Value::tagged(tag, "auth failed")).as_deref(),
                Some("auth failed")
            );
        }
        assert_eq!(
            cli_error_message(&Value::tagged("UICancelledError", "ignored")).as_deref(),
            Some("")
        );
    }

    #[test]
    fn record_error_data_preserves_primitives_and_debugs_complex_values() {
        let detail = Value::Object(Object::new().with("code", Value::Integer(500)));
        let error = Value::Object(
            Object::new()
                .with("code", Value::Integer(500))
                .with("retryable", Value::Boolean(false))
                .with("detail", detail.clone()),
        );
        let data = error_data(&error);

        assert_eq!(data.type_name, "Object");
        assert_eq!(
            data.properties,
            vec![
                ("code".to_string(), DataValue::Integer(500)),
                ("retryable".to_string(), DataValue::Boolean(false)),
                (
                    "detail".to_string(),
                    DataValue::Debug(format!("{detail:?}"))
                ),
                (
                    "message".to_string(),
                    DataValue::String(data.message.clone())
                ),
                (
                    "formatted".to_string(),
                    DataValue::String(data.formatted.clone())
                ),
            ]
        );
    }

    #[test]
    fn deep_cause_chain_terminates_without_recursive_calls() {
        let mut error = Value::Error(NativeError::new("Error", "leaf"));
        for index in 0..2_000 {
            error = Value::Error(
                NativeError::new("Error", &format!("layer-{index}")).with_cause(error),
            );
        }

        let message = error_message(&error);
        assert!(message.starts_with("layer-1999: cause: layer-1998"));
        assert!(message.ends_with(": cause: leaf"));
    }
}
