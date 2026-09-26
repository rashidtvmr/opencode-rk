# BRIDGE-PAR-286

claim: ses_par286 via tools/completion_claims.py
source: packages/tui/src/util/filetype.ts (LANGUAGE_EXTENSIONS + filetype collapse); crates/opentui-bridge/src/filetype.rs (full 118-key table, language_of)
observed: filetype.rs already covers full TS table; task wants tiny 10-variant subset standalone, no lib.rs/Cargo.toml edits
boundary: ONE new file crates/opentui-bridge/src/filetype_util_full.rs, std-only, forbid(unsafe_code), <100 lines, >=4 tests
tests: 5 tests in-file (maps_code_exts, maps_doc_exts, case_and_path, fallback_txt, is_code_split); verify rustfmt --check only (no cargo per task)
decisions: ext via basename rsplit + eq_ignore_ascii_case (no alloc); unknown/empty/dotfile/trailing-dot -> txt; is_code = rs|ts|tsx|js|sh|py
unknowns: none
