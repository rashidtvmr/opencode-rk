# V2 TOOLS-LIB mechanical Clippy candidate

- Package: `IMPLEMENTATION SOURCE-ONLY TOOLS-LIB-MECHANICAL-CLIPPY`
- Base: `7c39feef3a3282831522ea226708e6aee8ff847b`
- Scope: 15 explicitly granted `crates/tools/src` files plus this worklog.
- Contract: mechanical Clippy repairs only; no tests, manifests, lockfiles,
  SQL, configuration, shell-tool lifetime repair, or behavioral changes.

The changes replace lint-triggering equivalent forms (unused mutability,
identity maps, inclusive range checks, character-array search, `contains`,
`to_vec`, derived default, bounded `with_capacity`, `sort_by_key`, nested-if
collapse, direct returns, and single-character push). The two MCP spawn
functions retain their audited eight-argument capability/ownership/limit/
restart API and receive explicit `#[expect(clippy::too_many_arguments)]`.
`file_ops` keeps production writes on `open_write_root_at`; the two observer
helpers are test-only compatibility seams, with observer and hook aliases
preserving their existing types and borrows.

No Clippy/build/test command was run by this source-only worker. The parent
must independently preverify the candidate and rerun the combined gate after
the separate `shell_tool.rs` cancellation/lifetime repair. The known remaining
four shell-tool diagnostics are intentionally untouched, so this is a
CANDIDATE and not a whole-tools GREEN or ACCEPTED result.
