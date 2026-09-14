//! Tool schema definitions and validation.

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

/// Result of schema validation.
#[derive(Debug, Clone, PartialEq)]
pub enum ValidationResult {
    Valid,
    Invalid { errors: Vec<String> },
}

impl ValidationResult {
    /// Creates a valid result.
    #[must_use]
    pub fn valid() -> Self {
        Self::Valid
    }

    /// Creates an invalid result with the given errors.
    #[must_use]
    pub fn invalid(errors: Vec<String>) -> Self {
        Self::Invalid { errors }
    }

    /// Returns true if validation passed.
    #[must_use]
    pub fn is_valid(&self) -> bool {
        matches!(self, Self::Valid)
    }

    /// Returns true if validation failed.
    #[must_use]
    pub fn is_invalid(&self) -> bool {
        matches!(self, Self::Invalid { .. })
    }

    /// Returns the errors if validation failed.
    #[must_use]
    pub fn errors(&self) -> Option<&[String]> {
        match self {
            Self::Valid => None,
            Self::Invalid { errors } => Some(errors),
        }
    }
}

impl Default for ValidationResult {
    fn default() -> Self {
        Self::Valid
    }
}

/// Tool schema definition.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ToolSchema {
    /// Unique tool identifier.
    pub id: String,
    /// Human-readable tool name.
    pub name: String,
    /// Tool description.
    pub description: String,
    /// JSON Schema for input validation (draft-07).
    pub input_schema: Value,
    /// JSON Schema for output validation (draft-07).
    pub output_schema: Value,
    /// List of required field names in input.
    pub required_fields: Vec<String>,
    /// List of optional field names in input.
    pub optional_fields: Vec<String>,
}

impl ToolSchema {
    /// Creates a new empty tool schema.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates a tool schema with the given id and name.
    #[must_use]
    pub fn with_identity(id: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            ..Default::default()
        }
    }

    /// Returns true if the schema has a valid input schema definition.
    #[must_use]
    pub fn is_valid(&self) -> bool {
        !self.id.is_empty() && !self.name.is_empty() && self.input_schema.is_object()
    }
}

/// Schema validator for tool inputs and outputs.
#[derive(Clone, Debug)]
pub struct SchemaValidator {
    schema: ToolSchema,
}

impl SchemaValidator {
    /// Creates a new validator for the given schema.
    #[must_use]
    pub fn new(schema: ToolSchema) -> Self {
        Self { schema }
    }

    /// Validates input JSON against the schema.
    #[must_use]
    pub fn validate_input(&self, json: &Value) -> ValidationResult {
        if !json.is_object() {
            return ValidationResult::invalid(vec!["input must be a JSON object".to_string()]);
        }

        let obj = json.as_object().unwrap();
        let mut errors = Vec::new();

        // Check required fields
        for field in &self.schema.required_fields {
            if !obj.contains_key(field) {
                errors.push(format!("missing required field '{}'", field));
            }
        }

        // Validate against JSON Schema if present
        if let Some(schema_errors) = self.validate_against_schema(json, &self.schema.input_schema) {
            errors.extend(schema_errors);
        }

        if errors.is_empty() {
            ValidationResult::valid()
        } else {
            ValidationResult::invalid(errors)
        }
    }

    /// Validates output JSON against the schema.
    #[must_use]
    pub fn validate_output(&self, json: &Value) -> ValidationResult {
        self.validate_against_schema(json, &self.schema.output_schema)
            .map(|errors| {
                if errors.is_empty() {
                    ValidationResult::valid()
                } else {
                    ValidationResult::invalid(errors)
                }
            })
            .unwrap_or(ValidationResult::valid())
    }

    /// Validates a value against a JSON Schema.
    fn validate_against_schema(&self, value: &Value, schema: &Value) -> Option<Vec<String>> {
        if !schema.is_object() {
            return Some(Vec::new());
        }

        let schema_obj = schema.as_object()?;

        // Skip if no type defined
        if schema_obj.get("type").is_none() {
            return Some(Vec::new());
        }

        let mut errors = Vec::new();

        // Type validation
        if let Some(type_val) = schema_obj.get("type") {
            if let Some(type_str) = type_val.as_str() {
                if !self.matches_type(value, type_str) {
                    errors.push(format!(
                        "value type '{}' does not match schema type '{}'",
                        self.value_type(value),
                        type_str
                    ));
                }
            }
        }

        // Properties validation
        if let Some(props) = schema_obj.get("properties").and_then(|p| p.as_object()) {
            if let Some(obj) = value.as_object() {
                for (key, prop_schema) in props {
                    if let Some(val) = obj.get(key) {
                        if let Some(prop_errors) = self.validate_against_schema(val, prop_schema) {
                            errors.extend(prop_errors);
                        }
                    }
                }
            }
        }

        // Required fields validation
        if let Some(required) = schema_obj.get("required").and_then(|r| r.as_array()) {
            if let Some(obj) = value.as_object() {
                for req in required {
                    if let Some(req_str) = req.as_str() {
                        if !obj.contains_key(req_str) {
                            errors.push(format!("missing required field '{}'", req_str));
                        }
                    }
                }
            }
        }

        if errors.is_empty() {
            None
        } else {
            Some(errors)
        }
    }

    /// Checks if a value matches a JSON Schema type.
    fn matches_type(&self, value: &Value, type_str: &str) -> bool {
        match type_str {
            "string" => value.is_string(),
            "number" => value.is_number(),
            "integer" => value.is_number(),
            "boolean" => value.is_boolean(),
            "array" => value.is_array(),
            "object" => value.is_object(),
            "null" => value.is_null(),
            _ => true,
        }
    }

    /// Returns the JSON Schema type of a value.
    fn value_type(&self, value: &Value) -> &'static str {
        match value {
            Value::String(_) => "string",
            Value::Number(_) => "number",
            Value::Bool(_) => "boolean",
            Value::Array(_) => "array",
            Value::Object(_) => "object",
            Value::Null => "null",
        }
    }

    /// Returns a reference to the underlying schema.
    #[must_use]
    pub fn schema(&self) -> &ToolSchema {
        &self.schema
    }
}

impl Default for SchemaValidator {
    fn default() -> Self {
        Self::new(ToolSchema::new())
    }
}

/// Infers a JSON Schema from sample values.
pub fn infer_schema(samples: &[Value]) -> Value {
    if samples.is_empty() {
        return json!({ "type": "object" });
    }

    // Collect all property types from samples
    let mut property_types: std::collections::BTreeMap<
        String,
        std::collections::BTreeMap<String, bool>,
    > = std::collections::BTreeMap::new();

    for sample in samples {
        if let Some(obj) = sample.as_object() {
            for (key, value) in obj {
                let types = property_types.entry(key.clone()).or_default();
                let type_name = json_type(value);
                types.insert(type_name, true);
            }
        }
    }

    // Build properties object
    let mut properties = serde_json::Map::new();
    for (key, types) in &property_types {
        let mut prop_schema = serde_json::Map::new();

        if types.len() == 1 {
            prop_schema.insert(
                "type".to_string(),
                Value::String(types.keys().next().unwrap().clone()),
            );
        } else {
            prop_schema.insert(
                "type".to_string(),
                Value::Array(types.keys().cloned().map(Value::String).collect::<Vec<_>>()),
            );
        }

        properties.insert(key.clone(), Value::Object(prop_schema));
    }

    json!({
        "type": "object",
        "properties": properties,
        "additionalProperties": true
    })
}

/// Returns the JSON Schema type string for a value.
fn json_type(value: &Value) -> String {
    match value {
        Value::String(_) => "string".to_string(),
        Value::Number(n) => {
            if n.is_i64() || n.is_u64() {
                "integer".to_string()
            } else {
                "number".to_string()
            }
        }
        Value::Bool(_) => "boolean".to_string(),
        Value::Array(_) => "array".to_string(),
        Value::Object(_) => "object".to_string(),
        Value::Null => "null".to_string(),
    }
}

/// Creates a bash tool schema.
#[must_use]
pub fn bash_schema() -> ToolSchema {
    ToolSchema {
        id: "bash".to_string(),
        name: "Shell".to_string(),
        description: "Execute shell commands".to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "command": {
                    "type": "string",
                    "description": "Shell command to execute"
                },
                "args": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "Additional arguments"
                },
                "cwd": {
                    "type": "string",
                    "description": "Working directory"
                },
                "timeout_ms": {
                    "type": "integer",
                    "description": "Timeout in milliseconds"
                },
                "env": {
                    "type": "object",
                    "additionalProperties": { "type": "string" },
                    "description": "Environment variables"
                }
            },
            "required": ["command"]
        }),
        output_schema: json!({
            "type": "object",
            "properties": {
                "success": { "type": "boolean" },
                "stdout": { "type": "string" },
                "stderr": { "type": "string" },
                "exit_code": { "type": "integer" }
            }
        }),
        required_fields: vec!["command".to_string()],
        optional_fields: vec![
            "args".to_string(),
            "cwd".to_string(),
            "timeout_ms".to_string(),
            "env".to_string(),
        ],
    }
}

/// Creates a file tool schema.
#[must_use]
pub fn file_schema() -> ToolSchema {
    ToolSchema {
        id: "file".to_string(),
        name: "File Operations".to_string(),
        description: "Perform file operations".to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "File path"
                },
                "pattern": {
                    "type": "string",
                    "description": "Glob pattern for matching files"
                },
                "content": {
                    "type": "string",
                    "description": "Content to write"
                },
                "append": {
                    "type": "boolean",
                    "description": "Append to existing file"
                },
                "encoding": {
                    "type": "string",
                    "enum": ["utf-8", "binary"],
                    "description": "File encoding"
                }
            },
            "required": ["path"]
        }),
        output_schema: json!({
            "type": "object",
            "properties": {
                "path": { "type": "string" },
                "size": { "type": "integer" },
                "success": { "type": "boolean" }
            }
        }),
        required_fields: vec!["path".to_string()],
        optional_fields: vec![
            "pattern".to_string(),
            "content".to_string(),
            "append".to_string(),
            "encoding".to_string(),
        ],
    }
}

/// Creates a read tool schema.
#[must_use]
pub fn read_schema() -> ToolSchema {
    ToolSchema {
        id: "read".to_string(),
        name: "Read File".to_string(),
        description: "Read file contents".to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "File path to read"
                },
                "offset": {
                    "type": "integer",
                    "description": "Byte offset to start reading from"
                },
                "limit": {
                    "type": "integer",
                    "description": "Maximum bytes to read"
                },
                "encoding": {
                    "type": "string",
                    "enum": ["utf-8", "binary"],
                    "description": "File encoding"
                }
            },
            "required": ["path"]
        }),
        output_schema: json!({
            "type": "object",
            "properties": {
                "path": { "type": "string" },
                "content": { "type": "string" },
                "size": { "type": "integer" },
                "encoding": { "type": "string" }
            }
        }),
        required_fields: vec!["path".to_string()],
        optional_fields: vec![
            "offset".to_string(),
            "limit".to_string(),
            "encoding".to_string(),
        ],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bash_schema_valid() {
        let schema = bash_schema();
        assert!(schema.is_valid());
        assert_eq!(schema.id, "bash");
        assert_eq!(schema.name, "Shell");
        assert!(schema.required_fields.contains(&"command".to_string()));
        assert!(
            schema
                .input_schema
                .get("properties")
                .unwrap()
                .get("command")
                .unwrap()
                .get("type")
                .unwrap()
                .as_str()
                .unwrap()
                == "string"
        );
    }

    #[test]
    fn file_schema_valid() {
        let schema = file_schema();
        assert!(schema.is_valid());
        assert_eq!(schema.id, "file");
        assert_eq!(schema.name, "File Operations");
        assert!(schema.required_fields.contains(&"path".to_string()));
        assert!(
            schema
                .input_schema
                .get("properties")
                .unwrap()
                .get("path")
                .unwrap()
                .get("type")
                .unwrap()
                .as_str()
                .unwrap()
                == "string"
        );
    }

    #[test]
    fn validate_input_accepts_valid() {
        let schema = bash_schema();
        let validator = SchemaValidator::new(schema);

        let valid_input = json!({
            "command": "ls -la",
            "cwd": "/home/user",
            "timeout_ms": 5000
        });

        let result = validator.validate_input(&valid_input);
        assert!(result.is_valid());
    }

    #[test]
    fn validate_input_rejects_invalid() {
        let schema = bash_schema();
        let validator = SchemaValidator::new(schema);

        // Missing required field
        let invalid_input = json!({
            "cwd": "/home/user"
        });

        let result = validator.validate_input(&invalid_input);
        assert!(result.is_invalid());
        let errors = result.errors().unwrap();
        assert!(
            errors
                .iter()
                .any(|e| e.contains("missing required field 'command'"))
        );

        // Not an object
        let non_object = json!("not an object");
        let result = validator.validate_input(&non_object);
        assert!(result.is_invalid());
        let errors = result.errors().unwrap();
        assert!(!errors.is_empty());
    }

    #[test]
    fn infer_schema_from_samples() {
        let samples = vec![
            json!({"name": "Alice", "age": 30, "active": true}),
            json!({"name": "Bob", "age": 25, "active": false}),
            json!({"name": "Charlie", "age": 35, "active": true, "admin": false}),
        ];

        let inferred = infer_schema(&samples);

        // Should be        // Should be object type
        assert_eq!(inferred.get("type").unwrap(), "object");

        // Should have properties
        let props = inferred.get("properties").unwrap().as_object().unwrap();
        assert!(props.contains_key("name"));
        assert!(props.contains_key("age"));
        assert!(props.contains_key("active"));
        assert!(props.contains_key("admin"));

        // Name should be string type
        assert_eq!(
            props.get("name").unwrap().get("type").unwrap(),
            &Value::String("string".to_string())
        );

        // Age should be integer type
        assert_eq!(
            props.get("age").unwrap().get("type").unwrap(),
            &Value::String("integer".to_string())
        );

        // Active should be boolean type
        assert_eq!(
            props.get("active").unwrap().get("type").unwrap(),
            &Value::String("boolean".to_string())
        );

        // Empty samples should return basic object schema
        let empty_schema = infer_schema(&[]);
        assert_eq!(empty_schema.get("type").unwrap(), "object");
    }

    #[test]
    fn schema_validator_validates_output() {
        let schema = bash_schema();
        let validator = SchemaValidator::new(schema);

        let output = json!({
            "success": true,
            "stdout": "hello",
            "stderr": "",
            "exit_code": 0
        });

        let result = validator.validate_output(&output);
        assert!(result.is_valid());
    }

    #[test]
    fn tool_schema_default_is_empty() {
        let schema = ToolSchema::new();
        assert!(schema.id.is_empty());
        assert!(schema.name.is_empty());
        assert!(schema.required_fields.is_empty());
        assert!(schema.optional_fields.is_empty());
    }
}
