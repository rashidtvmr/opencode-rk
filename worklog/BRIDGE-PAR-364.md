# BRIDGE-PAR-364 scratchpad (UNCLAIMED - orchestrator owns claims.json)

Task: new file `crates/opentui-bridge/src/ctx_args_full.rs`, CtxArgs bounded vec.
Status: file written, rustfmt-only verification. No ledger claim (per task orders).

## Claim
Unclaimed by design. Did not touch tasks/completion/claims.json.

## Source evidence
- TS truth `packages/tui/src/context/args.tsx:1-16`: `createSimpleContext({name:"Args",init:(props:Args)=>props})`, Args props model/agent/prompt/sessionID strings + continue/fork/auto bools. File only 16 lines (no 50).
- Pattern `crates/opentui-bridge/src/args_ctx.rs:1-132`: existing ArgsCtx caps 64/512 + cwd; style mirror (forbid unsafe, char-take truncation, push/get/len).
- Style ref `exit_ctx.rs:1-30`: char-safe cap note.

## Target boundary
ONE file only. No lib.rs, no Cargo.toml, no cargo, no commit.

## Tests (4, inline #[cfg(test)])
push_get_roundtrip, oob_is_none, cap_rejects, truncates. 88 lines total.

## Decisions
- Caps per spec: 32 args, 256 chars each (differs from args_ctx 64/512; separate full-mirror type).
- Char-count truncation (not byte slice) to avoid multibyte panic.
- `args` field pub per deliverable struct shape.

## Unknowns
- Wiring into lib.rs left to orchestrator (out of scope).
