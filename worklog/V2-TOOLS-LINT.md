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

## Preservation evidence

The protected in-module test-region SHA-256 values were compared byte-for-byte
against base `7c39feef3a3282831522ea226708e6aee8ff847b`; unchanged values are
listed as `before == after`, and files without an in-module `#[cfg(test)] mod
tests` are explicitly `none`:

| file | protected region SHA-256 (before == after) |
| --- | --- |
| `executor.rs` | `c0627964a830710a03f0758a91b0ccb796a13ba669ed501d66ea3d0ee13ac373` |
| `file_ops.rs` | `ea5fcab13496c480015a45d2abc97679fe43f343a9441ea63c5cf576f29126ee` |
| `git_lane.rs` | `19e6e4639406fac6b7c3a361f19bf36cb73f1f44951dd20d78420f85ab88e2dd` |
| `hooks_bridge.rs` | `none` |
| `lsp_client.rs` | `1f2062b8fbea99732f211be9bf9b2d63f745a50f8c658fca7e7d9b1bdacf4dc5` |
| `mcp_catalog_search.rs` | `none` |
| `mcp_spawn.rs` | `86723f843bbce81dc85d7de3d72f7ddce44e10019cf1ac34d9835702352f77e6` |
| `registry_dispatch.rs` | `1d804e57c0e0bf11c29b3b8fae1fb9b1e8f2f631a911c61a418ee75bf77b54be` |
| `rtk_core.rs` | `none` |
| `rtk_testfilter.rs` | `none` |
| `schema.rs` | `2cc0b255c5b85859d8f85fc66790fb1aa87260637e82a4154fb7d04e29d2e2d9` |
| `shell_bounds.rs` | `64714b46c5ae887d48f96894035fa139cfd23a84ba611ae350374cc6c3927d9f` |
| `terse_render.rs` | `none` |
| `tool_index.rs` | `none` |
| `tool_query.rs` | `none` |

Protected global fixture/config evidence: `Cargo.lock` SHA-256 is
`63ef5299dd93286950af00388796375b06aefc5a4a3eedfa38361954fefb6f03` (base
and candidate); no manifests, SQL, fixtures, or test files changed. The
comparison located the actual `#[cfg(test)] mod tests` marker, rather than
mistaking test-only imports or helper code for the protected test body.

Final source-only correction: `RootObserver<'_>` is also used by the
production `open_write_root_at` signature, eliminating the remaining
`clippy::type_complexity` diagnostic while preserving the exact `FnMut` borrow
and single observer invocation. The two current-working-directory observer
compatibility wrappers remain test-only. Protected test-region hashes above
remain unchanged.

## Exact integrated acceptance

The controller prepared preserved `3b43b20`, `cf6e962` and `2e59657` source
against accepted canonical `a6b188e`, alongside the separately frozen shell
ownership repair. Independent fixture maintenance replaces only the absent
macOS `/bin/false` pathname in the MCP crash case, preserving every assertion;
the companion worklog records original and corrected module hashes. Every other
protected test region and Cargo.lock remain unchanged.

Independent verifier `ses_f0ab46518ffepDee7uyTjYb2YY` PREVERIFIED complete
candidate **`e1acd5258fed50b1f79f54b412921266f07dc8ed`**. The same bounded
isolated commands pass after fast-forward integration on that exact SHA:

- Formatting: exit 0.
- Tools-library Clippy with `-D warnings`: exit 0, including the separately
  repaired four shell-file diagnostics.
- Frozen real-process ownership target: five pass, zero failed/ignored,
  no forced cleanup.
- Full all-target/all-feature tools crate: **458 pass, zero failed/ignored**,
  exit 0.

Receipt roots: `v2-tools-owned-preverify-e1acd52-swt6njq0` and
`v2-tools-owned-integrated-e1acd52-mg5gxwlw` under the approved artifact parent.
State: **ACCEPTED for tools-library mechanical diagnostics on exact integrated
`e1acd52`**. The mechanical package remains the 15 declared source paths;
ownership and fixture maintenance are separately documented. Workspace-wide
Clippy and tests remain required.
