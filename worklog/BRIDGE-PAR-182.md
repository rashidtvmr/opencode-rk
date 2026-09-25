# Claim BRIDGE-PAR-182 ses_par182
- Source: `crates/opentui-bridge/src/session_shared_full.rs:16-20` SessionSharedFull{id,title,msgs}; `:49-52` header(); `crates/opentui-bridge/src/session_index.rs:16-20` SessionIndex{foreground_tasks,actions,kv_hide}
- Target: ONE file `crates/opentui-bridge/src/session_header.rs`, no lib.rs/Cargo.toml edits, no cargo
- Tests: in-file unit tests (>=4), verify `rustfmt --check` only
- Decisions: title_or_id = title if non-empty else id; header_lines = [clip(sess.header), clip(tasks N actions M)], char-safe width clip
- Status: file written, rustfmt clean
