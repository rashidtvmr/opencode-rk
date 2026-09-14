# TOOL-002: Tool Schema and Validation Implementation

## Scope
- Only file: `crates/tools/src/schema.rs`

## Deliverable
Implementation of:
- `ToolSchema` struct with fields: id, name, description, input_schema, output_schema, required_fields, optional_fields
- `SchemaValidator` struct with methods: validate_input(json), validate_output(json), infer_schema(samples)
- Built-in templates: `bash_schema()`, `file_schema()`, `read_schema()`
- 5 tests: bash_schema_valid, file_schema_valid, validate_input_accepts_valid, validate_input_rejects_invalid, infer_schema_from_samples

## Context
- Existing code in crates/tools/src/schema.rs
- serde_json for JSON handling
- JSON Schema draft-07 validation

## Constraints
- No unsafe code (enforced at crate level)
- Must implement type validation and property validation
- infer_schema must handle empty samples gracefully

## Verification
Run:
```bash
cargo test -p opencode-rk-tools
cargo check --workspace
```

## Status
Implementation complete. Already present in schema.rs (lines 1-631).