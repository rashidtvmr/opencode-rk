# AGENT-MODE-ROLE — subagent mode + role prompting from .agents/ files

Session: ses_f1ea760dcffehvM9O8UnrIdRxf
Branch: agent/fix-20260926 (worktree, off 96087d1 = WIP of local agent-loop tree)

## Claim
Wire agent-mode / sub-agent role prompting from `.agents/` files into agent
execution. Audit showed the loader exists but is dead code.

## Source evidence (at 96087d1)
- `crates/agents/src/agent_files.rs:294` `load_agent_files` — full loader
  (md frontmatter + json, bounds, symlink escape reject). Tests exist via
  `#[path]` include: `crates/agents/tests/agent_files.rs`.
- `crates/agents/src/lib.rs:5-11` — module list: agent_files NOT declared →
  module is orphaned from the crate; only reachable standalone via `#[path]`.
- `crates/agents/Cargo.toml` — serde_json only in dev-dependencies, but
  agent_files.rs uses it in non-test code → crate build would fail if wired.
- `crates/server/src/lib.rs:733-737` `CreateTurnBody` — no `agent` field.
- `crates/server/src/lib.rs:872` `create_turn_stream` — sends session history
  only; no system/role item; no agent definitions consulted anywhere.
- `crates/server/src/remote_turns.rs:32` `ALLOWED_AGENTS` allowlist — agent
  name validated for phone submissions but never used to alter the prompt.
- No `.agents/agents/` directory exists; repo has only `.agents/WORKER.md`.
- Nothing in crates/ scans `.agents/`.

## Observable contract (new behavior)
1. `agent_files::AgentDef::role_prompt()` — build system/role text: optional
   mode directive ("plan" => may not modify; else none) + body text.
2. `agent_files::find(snapshot, name)` — deterministic lookup by name.
3. `agent_files::advertised_tools(def, enabled)` — intersection of the agent
   file tool allowlist with the server-enabled turn tools; empty allowlist =
   all enabled; `mode: plan` => no executable tools (read/advertise none).
4. `crates/agents` lib declares `pub mod agent_files;` (serde_json promoted
   to real dependency).
5. `create_turn_stream`: optional `agent` field in body; when an agent file in
   `<cwd>/.agents/agents/` matches the name, prepend a `System`
   `ResponsesItem` with `role_prompt()` and restrict advertised tools via
   `advertised_tools`. Fail-open to normal turn when directory absent or name
   unknown (agent field is an optimization, unknown => bad request ONLY when
   a definitions dir exists and name is not in it — decided: reject unknown
   name when dir exists, else ignore).
6. Model/temperature from agent files: NOT applied in this slice (server
   trusts client-sent model; recorded as remaining unknown).

## Tests (RED first, then frozen)
- `crates/agents/tests/agent_role.rs` (`#[path]` include): t01 role prompt
  build/plan directive, t02 find, t03 tool intersection, t04 plan mode =>
  empty advertised tools, t05 empty allowlist passthrough.

## Decisions
- Keep decision logic pure and in the agents crate; server just calls it.
- No new env vars; discovery root = current_dir()/.agents/agents.

## Remaining unknowns
- Whether phone-submission Selection.agent should map to these definitions
  (remote_turns currently validates "general|coder|reviewer" only).
- Per-agent model/temperature override into provider request body.
- Non-streaming `create_turn` accepts but ignores `agent` (no-op there); only
  the live `/turns/stream` path applies role prompting.

## Verification record
- agents RED: compile failure (role_prompt/find/advertised_tools missing),
  frozen test sha256 head `5fa5e6486d03b95b`.
- server RED: compiling stub, behavioral 2 pass / 3 fail, frozen test sha256
  head `a1745c5149a35754`. Zero test edits after freeze.
- GREEN: `cargo test -p opencode-rk-agents` 131/131 (12 suites, incl. new
  agent_role 5/5 and pre-wired `pub mod agent_files`); `cargo test -p
  opencode-rk-server --test agent_roles` 5/5; `--lib` 208/208;
  `agent_loop_turns` 1/1 (deny-default turn behavior unchanged when no agent).
- serde_json promoted agents dep (loader used it in non-test code already).
