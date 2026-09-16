# INT-VERIFY4 (verify-only, no stubs)

Rev: `248f519d53ed29cbf7fef7ceb2b5010fb1a4224b`.
Dirty-on-arrival: y (204 M + 221 untracked on arrival; count 421-425 during run).
Owned lanes: INT-001/002/003/005/006/007/009/010.
Frozen untouched: `lib.rs` clean (`git diff --name-only HEAD -- lib.rs ralph.json` empty), `ralph.json` clean.
Serial: `CARGO_BUILD_JOBS=1`, THREADS=1, `timeout 120`, `free -h` checked (~2.6-2.9 Gi avail each run).
No product/test edits made.

## sha256 (product + test, at verify time)

- INT-001 `int_registry_lane.rs` prod `bcac9019356f7304bab80b8abb0bf9799673b2c43dfbee0330d4b39746544228` test `6fd83d47f61fed6db064bc14dd2a1a5b7104598a164b5d62a586913a134ce05a`
- INT-002 `int_connect.rs` prod `5b19f2df62ef9730cfe13e65d9fc1dc5ee26d1e8de8b2d266e67e29df8376629` test `2494e3c7285dd62788136477d7a1a5d81667a17d7c963cfb4a9f683866f5a68b`
- INT-003 `int_methods.rs` prod `582e8d08c5086b6ed0bf7a4f844793951de5efd8a60b05e5225200e3a616a9eb` test `a08c0bea0b4c6dacd48f6586e7f03e970768e36e46a564a320cdf016c3da3b6c`
- INT-005 `int_refresh.rs` prod `75aa9c28bce8ddc5ff4322ddf1faa821b170e80236802a9c72dda60fb69564bb` test `05083e052271f116627b7f9cbb9a1f400c63100627d75fd9a108e7af2248fc48`
- INT-006 `mcp_transport.rs` prod `b9549a0cacf79ebd4172570c066dc147c714a9253b33edc1a8baaea0a82c2c5b` test `2b30d993df2c55746d12da437d1ecaaf83f6a8a8b82afa5973c643b3a097af36`
- INT-007 `handler_projection.rs` prod `1a39f608b02d3258b973a2af54b349772f164c9e342210e01d105b89101240ec` test `260d5b1bfe8b31f9ea4a6579c2d18a8e9efd8f582059d98d54bda6a522cc0425`
- INT-009 `location_ctx.rs` prod `d97f2f79cd5852f36f0b65cb9453292f4b82c2ed3a430af1db9dfb2e7a56f25b` test `638f265fb6b3515b1aabbcfa313629880b13396661c8560ba5b184662959e1d9`
- INT-010 `share_descriptor.rs` prod `627f783f3ccd0e1172aef78fa3edc3ec796dffcdaf365cdf255c7ffb390ed0b7` test `fcd0c112d9fc4f5e0446dfc696cfee37bf7e737ed35ca9c2886717dcd2ebb7f7`

Note: hashes differ from some per-lane worklogs (INT-001/002/003/005/006/009/010).
Lane worklogs record earlier states; tree dirty from parallel lanes. Verify-only:
hashes above are evidence-at-verify-time, no freeze claimed.

## Stub scan

`grep -rn 'todo!\|unimplemented!'` over all 8 product files: exit 1, zero hits.
No stubs. Real code wired via `crates/providers/src/lib.rs` (pre-existing wiring,
untouched).

## Results (serial, JOBS=1)

Each suite run serially 5/5, then aggregate:
`cargo test -p opencode-rk-providers --test int_registry_lane --test int_connect
--test int_methods --test int_refresh --test mcp_transport --test handler_projection
--test location_ctx --test share_descriptor` => RC 0.
Log: `/tmp/opencode/yE-int.log` — 8x `test result: ok. 5 passed; 0 failed`.
Total: 8 suites, 40/40 pass, 0 fail.

| suite | tests | pass |
|---|---|---|
| int_registry_lane (INT-001) | 5 | 5 |
| int_connect (INT-002) | 5 | 5 |
| int_methods (INT-003) | 5 | 5 |
| int_refresh (INT-005) | 5 | 5 |
| mcp_transport (INT-006) | 5 | 5 |
| handler_projection (INT-007) | 5 | 5 |
| location_ctx (INT-009) | 5 | 5 |
| share_descriptor (INT-010) | 5 | 5 |

RED not re-probed (verify-only; mutation RED evidence lives in per-lane worklogs
INT-005/006/007). No `passes:true` claimed beyond log counts.
