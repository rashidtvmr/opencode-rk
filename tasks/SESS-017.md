# SESS-017

Status: TO_DO. Kind: product. Runtime optional: False.
Mandatory for full declared release: yes.
Requirements: None.
Dependencies: crates/contracts provides base types.

## User-observable outcome

Session import functionality supporting JSON, CSV, and plain text formats.

## Source evidence

- SessionId is defined in `crates/contracts/src/lib.rs:59-60`.
- Task specifies ImportResult, SessionImporter, ImportFormat in import.rs.

## Observable contract

- `ImportResult` struct with fields: `success: bool`, `session_id: Option<SessionId>`, `error: Option<String>`.
- `ImportFormat` enum with variants: `Json`, `Csv`, `PlainText`.
- `SessionImporter` struct with fields: `format: ImportFormat`, `allow_duplicates: bool`, `default_title: Option<String>`.
- Methods: `import(data: &str) -> Result<ImportResult, ImportError>`, `import_from_file(path)`.
- Builder methods: `with_allow_duplicates(bool)`, `with_default_title(Option<String>)`.

## Test obligations

- SESS-017-T01: import_json_valid - valid JSON session imports successfully.
- SESS-017-T02: import_csv_valid - valid CSV session imports successfully.
- SESS-017-T03: import_text_valid - valid plain text session imports successfully.
- SESS-017-T04: duplicates_handled - allow_duplicates flag is respected.
- SESS-017-T05: error_invalid_format - invalid JSON/UUID returns ImportError.

## Verification

```bash
cargo test -p opencode-rk-sessions
cargo check --workspace
```

Both commands pass with no errors.