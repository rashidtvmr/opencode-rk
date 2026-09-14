# TOOL-002 - Tool schema definitions and validation

Status: IN PROGRESS. Kind: product. Runtime optional: False.
Mandatory for full declared release: yes.
Requirements: REQ-037.
Dependencies: BASE-002 (Event contracts).

## User-observable outcome

ToolSchema struct with id, name, description, input_schema (JSON Schema), output_schema, required_fields, optional_fields.
SchemaValidator with validate_input(json), validate_output(json), infer_schema(samples).
Built-in schema templates: bash_schema, file_schema, read_schema.

## Source evidence

- claude-code Tool schemas and JSON Schema definitions.
- OpenCode V2 tool_catalog spec.
- crates/contracts/src/lib.rs (serde, Serialize/Deserialize patterns).

## Observable contract

### ToolSchema
- id: String (tool identifier)
- name: String (human-readable name)
- description: String
- input_schema: serde_json::Value (JSON Schema draft-07)
- output_schema: serde_json::Value (JSON Schema draft-07)
- required_fields: Vec<String> (top-level required property names)
- optional_fields: Vec<String> (top-level optional property names)

### SchemaValidator
- New constructor that accepts ToolSchema.
- validate_input(json: &serde_json::Value) -> ValidationResult
- validate_output(json: &serde_json::Value) -> ValidationResult
- infer_schema(samples: &[serde_json::Value]) -> serde_json::Value

### Built-in schema templates
- bash_schema() -> ToolSchema (command, args arrays, cwd, timeout)
- file_schema() -> ToolSchema (path, pattern, content)
- read_schema() -> ToolSchema (path, offset, limit, encoding)

## Failure states
- ValidationResult::Invalid with Vec<String> of error messages.
- Inference on empty samples returns empty schema.
- Malformed JSON in input/output returns validation error.

## Acceptance criteria
- cargo test -p opencode-rk-tools TOOL-002 tests pass (5 tests).
- cargo check --workspace clean.
- cargo fmt applied.

## Test obligations
- TOOL-002-T01: bash_schema_valid - bash_schema() returns valid schema.
- TOOL-002-T02: file_schema_valid - file_schema() returns valid schema.
- TOOL-002-T03: validate_input_accepts_valid - validates correct input.
- TOOL-002-T04: validate_input_rejects_invalid - rejects malformed input.
- TOOL-002-T05: infer_schema_from_samples - infers schema from JSON samples.