# APP-012-SECURE-DISPATCH-W1

- Claim: APP-012-SECURE-DISPATCH-W1; session ses_f1e281c6cffeB1mW1cW4Lde0dT; base 49d3e29634b4ba742dec4ee96d6a23ea21558ef1.
- Scope: crates/server/src/lib.rs only for product code; frozen tests immutable.
- Contract: parse bounded write path/content/append from tool arguments; authorize through FileTool::execute_authorized and the existing PermissionBroker/SecurityPolicy rooted at current project; denied or human-required writes perform zero I/O and emit bounded truthful output; allowed writes preserve exact bytes, emit no secret path/content, and continue provider once within existing loop/cancellation/resource bounds.
- Source evidence: crates/server/src/lib.rs:1143-1215 currently dispatched every call through generic ToolExecutor, producing `Unknown tool: write`; crates/tools/src/file_ops.rs:154-200 exposes FileTool::execute_authorized with broker-before-I/O semantics; crates/security/src/lib.rs:243-288 applies FileAction::Write root/secret/special-path policy; crates/server/src/agent_loop.rs:21-30 bounds rounds, calls, output.
- Implementation: crates/server/src/lib.rs parses bounded `path`, `content`, optional boolean `append`; dispatches `write` through FileTool::execute_authorized; emits generic bounded success/denial/failure text; redacts write arguments from public `tool_call` NDJSON while retaining private provider replay arguments; leaves generic executor path unchanged for existing tools.
- Observable boundary: parser max 1 MiB arguments/content, 4096-byte path; deny/human results perform no file I/O and reveal neither path nor payload; allowed result reports only `write success`; existing loop emits exactly one bounded continuation under LoopController.
- Tests: frozen denial 0366416c4863940221c74aac08deeeba3c9404cd92ff18c9411d2cab5b570829; in-root 6e3841384c7a477ed226f0bf5e28f5b40e06fe683fc9683059e699fd3768aa12; symlink fe8d24946c8ce98aae095b6a2f92b7f3c19cc32532ae6263f9898d8a5699753c.
- Verification: dispatch, in-root, symlink, and `agent_loop_turns` each passed with `CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1`; hashes rechecked after tests.
- Remaining: convergence gate remains blocked by pre-existing ledger findings; parent APP-012 TOCTOU repair remains open.
- Status: candidate GREEN; awaiting independent integration acceptance.
