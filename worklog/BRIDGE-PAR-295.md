# BRIDGE-PAR-295 locale_util_full

Claim: ses_par295 via completion_claims.claim, ok.
Source evidence:
- TS truth /home/rashid/projects/opencode/packages/tui/src/util/locale.ts:81-84 pluralize (count===1 singular, `{}` fill); :30-37 number K/M abbrev.
- Sibling crates/opentui-bridge/src/locale.rs:149-152 pluralize(i64, template fill); :83-93 number(f64).
- Sibling crates/opentui-bridge/src/format_util_full.rs:38-40 plural(u64, one/many).
Observed: locale.rs covers template-fill pluralize; no thousands-comma formatter, no `N word` pluralize(u64), no lang subtag helper.
Target boundary: ONE new file crates/opentui-bridge/src/locale_util_full.rs. No lib.rs/Cargo.toml edits.
Tests: 4 in-file #[cfg(test)]: commas_grouped, commas_negative_and_min (incl i64::MIN), pluralize_forms, lang_of_splits.
Decisions: format_number via unsigned_abs+reverse-group (MIN-safe); pluralize returns "N word" (matches format_util_full::plural shape, distinct name per spec); lang_of slices to first -/_ (unicode-boundary safe: -/_ are ASCII). std-only, forbid(unsafe_code), 79 lines.
Unknowns: none. Verification: rustfmt --check PASS (no cargo per task).
