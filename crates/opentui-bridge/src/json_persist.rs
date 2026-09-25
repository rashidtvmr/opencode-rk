#![forbid(unsafe_code)]
//! Atomic JSON persistence (mirrors `util/persistence.ts:22` `writeJsonAtomic`).
//!
//! TS notes: `writeJsonAtomic` = `JSON.stringify` (caller side) + tmp-file in
//! the same dir + rename, pid+uuid tmp name, tmp cleanup on failure. The Rust
//! caller supplies already-serialized JSON text (`&str`); this module only
//! validates shape, never parses (std only, no serde).
//!
//! Validation (fail-closed, no write on error):
//! - non-empty after trim; first non-ws char must be `{` or `[`, and the
//!   balance scan must close the outer pair with only whitespace after it;
//! - `{}`/`[]` balance scan that respects `"..."` string literals and `\`
//!   escapes; mismatched or unbalanced nesting is rejected.
//!
//! IO reuses `crate::persistence` (atomic tmp+rename `write_text`, 1 MiB cap,
//! `read_text` for read-back). Oversize input errors before touching disk.

use std::path::Path;

use crate::persistence::{read_text, write_text, PersistError, MAX_READ_BYTES};

/// Max bytes accepted for one JSON document (same 1 MiB cap as persistence).
pub use crate::persistence::MAX_READ_BYTES as MAX_JSON_BYTES;

/// Validate shape, then atomic-write via `persistence::write_text`.
///
/// Errors (nothing written): empty/whitespace-only, wrong outer shape,
/// unbalanced/mismatched braces/brackets, or input over [`MAX_JSON_BYTES`].
pub fn write_json_atomic(path: &Path, json_text: &str) -> Result<(), PersistError> {
    if json_text.len() > MAX_READ_BYTES {
        return Err(PersistError::TooLarge);
    }
    validate_json_shape(json_text)?;
    write_text(path, json_text)
}

/// Read back JSON text via `persistence::read_text`, re-validating shape.
///
/// A file mutated to non-JSON after write fails with `JsonShape` instead of
/// returning bad data.
pub fn read_json_atomic(path: &Path) -> Result<String, PersistError> {
    let text = read_text(path)?;
    validate_json_shape(&text)?;
    Ok(text)
}

/// Fail-closed shape check: non-empty, `{...}`/`[...]` outer pair, balanced
/// nesting outside string literals.
fn validate_json_shape(text: &str) -> Result<(), PersistError> {
    let t = text.trim();
    if t.is_empty() {
        return Err(PersistError::JsonShape("empty document".to_string()));
    }
    let mut chars = t.chars();
    let first = chars.next().unwrap(); // non-empty checked above
    if first != '{' && first != '[' {
        return Err(PersistError::JsonShape(format!(
            "expected '{{' or '[', got {first:?}"
        )));
    }
    // NOTE: no ends_with close check here: a doc may legally end with '"'
    // (e.g. {"s":"a}b]"}); the balance scan below enforces the outer pair.
    let mut stack: Vec<char> = Vec::new();
    let mut in_str = false;
    let mut escaped = false;
    let chars: Vec<char> = t.chars().collect();
    for (i, c) in chars.iter().enumerate() {
        if in_str {
            if escaped {
                escaped = false;
            } else if *c == '\\' {
                escaped = true;
            } else if *c == '"' {
                in_str = false;
            }
            continue;
        }
        match c {
            '"' => in_str = true,
            '{' | '[' => stack.push(*c),
            '}' => {
                if stack.pop() != Some('{') {
                    return Err(PersistError::JsonShape("mismatched '}'".to_string()));
                }
            }
            ']' => {
                if stack.pop() != Some('[') {
                    return Err(PersistError::JsonShape("mismatched ']'".to_string()));
                }
            }
            _ => {}
        }
        if stack.is_empty() {
            // First top-level value closed early: only trailing ws may follow.
            let rest: String = chars[i + 1..].iter().collect();
            if !rest.trim().is_empty() {
                return Err(PersistError::JsonShape(
                    "trailing characters after JSON value".to_string(),
                ));
            }
            break;
        }
    }
    if in_str {
        return Err(PersistError::JsonShape("unterminated string".to_string()));
    }
    if !stack.is_empty() {
        return Err(PersistError::JsonShape("unbalanced brackets".to_string()));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    static N: AtomicU64 = AtomicU64::new(0);

    fn tmp_file(tag: &str) -> std::path::PathBuf {
        let n = N.fetch_add(1, Ordering::Relaxed);
        std::env::temp_dir().join(format!("gap06-{tag}-{}.{n}.json", std::process::id()))
    }

    #[test]
    fn roundtrip_object_and_array() {
        for doc in [r#"{"k":1,"s":"a}b]"}"#, "[1,2,{\"a\":[]}]"] {
            let p = tmp_file("roundtrip");
            let _ = std::fs::remove_file(&p);
            write_json_atomic(&p, doc).unwrap();
            assert_eq!(read_json_atomic(&p).unwrap(), doc);
            let _ = std::fs::remove_file(&p);
        }
    }

    #[test]
    fn unbalanced_rejected_no_file() {
        for doc in [
            "{\"k\":1",
            "[1,2",
            "{\"a\":[1}",
            "{\"a\"]}",
            "}{",
            "]",
            "}",
            "{} trailing",
            "{} {}",
            "plain text",
            "null",
        ] {
            let p = tmp_file("unbalanced");
            let _ = std::fs::remove_file(&p);
            assert!(
                matches!(write_json_atomic(&p, doc), Err(PersistError::JsonShape(_))),
                "must reject {doc:?}"
            );
            assert!(!p.exists(), "rejected write must not create file");
        }
    }

    #[test]
    fn empty_rejected_no_file() {
        for doc in ["", "   ", "\n\t "] {
            let p = tmp_file("empty");
            let _ = std::fs::remove_file(&p);
            assert_eq!(
                write_json_atomic(&p, doc),
                Err(PersistError::JsonShape("empty document".to_string()))
            );
            assert!(!p.exists(), "empty write must not create file");
        }
    }

    #[test]
    fn oversize_rejected_no_file() {
        let p = tmp_file("oversize");
        let _ = std::fs::remove_file(&p);
        // Over cap errors before touching disk even if shape looks valid.
        let mut big = String::from("{\"k\":\"");
        big.push_str(&"x".repeat(MAX_JSON_BYTES));
        big.push_str("\"}");
        assert_eq!(write_json_atomic(&p, &big), Err(PersistError::TooLarge));
        assert!(!p.exists(), "oversize write must not create file");
    }

    #[test]
    fn failed_write_leaves_original_intact() {
        let p = tmp_file("atomic");
        let _ = std::fs::remove_file(&p);
        write_json_atomic(&p, r#"{"v":1}"#).unwrap();
        assert!(matches!(
            write_json_atomic(&p, "{\"v\":"),
            Err(PersistError::JsonShape(_))
        ));
        assert!(matches!(
            write_json_atomic(&p, ""),
            Err(PersistError::JsonShape(_))
        ));
        assert_eq!(read_json_atomic(&p).unwrap(), r#"{"v":1}"#);
        // No stray tmp files beside the target.
        let dir = p.parent().unwrap();
        let stem = p.file_name().unwrap().to_string_lossy().into_owned();
        let strays: Vec<_> = std::fs::read_dir(dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .filter(|n| n.starts_with(&stem) && *n != stem)
            .collect();
        assert!(strays.is_empty(), "stray tmps: {strays:?}");
        let _ = std::fs::remove_file(&p);
    }
}
