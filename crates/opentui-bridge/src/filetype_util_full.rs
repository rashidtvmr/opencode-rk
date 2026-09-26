#![forbid(unsafe_code)]
//! Minimal path -> filetype subset (10 variants).
//!
//! TS truth: `packages/tui/src/util/filetype.ts` full table lives in
//! `filetype.rs`; this file is the tiny subset for callers needing only
//! `rs|ts|tsx|js|md|json|toml|sh|py|txt`.
// ponytail: subset only; upgrade: delegate to `filetype::language_of`.

/// Filetype id by extension (lowercase basename suffix), else `"txt"`.
#[must_use]
pub fn filetype_of(path: &str) -> &'static str {
    let base = path.rsplit(['/', '\\']).next().unwrap_or(path);
    let ext = match base.rsplit_once('.') {
        Some((_, e)) if !e.is_empty() => e,
        _ => return "txt",
    };
    if ext.eq_ignore_ascii_case("rs") {
        "rs"
    } else if ext.eq_ignore_ascii_case("ts") {
        "ts"
    } else if ext.eq_ignore_ascii_case("tsx") {
        "tsx"
    } else if ext.eq_ignore_ascii_case("js") {
        "js"
    } else if ext.eq_ignore_ascii_case("md") {
        "md"
    } else if ext.eq_ignore_ascii_case("json") {
        "json"
    } else if ext.eq_ignore_ascii_case("toml") {
        "toml"
    } else if ext.eq_ignore_ascii_case("sh") {
        "sh"
    } else if ext.eq_ignore_ascii_case("py") {
        "py"
    } else {
        "txt"
    }
}

/// True for code types (`rs|ts|tsx|js|sh|py`), false for doc/data/text.
#[must_use]
pub fn is_code(path: &str) -> bool {
    matches!(filetype_of(path), "rs" | "ts" | "tsx" | "js" | "sh" | "py")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_code_exts() {
        assert_eq!(filetype_of("main.rs"), "rs");
        assert_eq!(filetype_of("a.ts"), "ts");
        assert_eq!(filetype_of("b.tsx"), "tsx");
        assert_eq!(filetype_of("c.js"), "js");
        assert_eq!(filetype_of("s.sh"), "sh");
        assert_eq!(filetype_of("x.py"), "py");
    }

    #[test]
    fn maps_doc_exts() {
        assert_eq!(filetype_of("README.md"), "md");
        assert_eq!(filetype_of("c.json"), "json");
        assert_eq!(filetype_of("C.toml"), "toml");
        assert_eq!(filetype_of("n.txt"), "txt");
    }

    #[test]
    fn case_and_path_insensitive() {
        assert_eq!(filetype_of("src/App.TSX"), "tsx");
        assert_eq!(filetype_of("C:\\src\\main.RS"), "rs");
        assert_eq!(filetype_of("/tmp/x.PY"), "py");
    }

    #[test]
    fn fallback_txt() {
        assert_eq!(filetype_of(""), "txt");
        assert_eq!(filetype_of("Makefile"), "txt");
        assert_eq!(filetype_of(".gitignore"), "txt");
        assert_eq!(filetype_of("foo.xyz"), "txt");
        assert_eq!(filetype_of("trailing."), "txt");
    }

    #[test]
    fn is_code_split() {
        assert!(is_code("main.rs"));
        assert!(is_code("a.tsx"));
        assert!(!is_code("README.md"));
        assert!(!is_code("c.json"));
        assert!(!is_code("C.toml"));
        assert!(!is_code("n.txt"));
    }
}
