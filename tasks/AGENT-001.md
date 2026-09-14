# AGENT-001

Status: NOT STARTED. Kind: product. Runtime optional: False.
Mandatory for full declared release: yes.
Dependencies: none.
Test obligations: AGENT-001-T01, AGENT-001-T02, AGENT-001-T03, AGENT-001-T04, AGENT-001-T05.

## User-observable outcome

In-memory agent registry supporting registration, lookup, listing, and removal
of agents keyed by a typed `AgentId` derived from `opencode-rk-contracts`.

## Source evidence

- `crates/contracts/src/lib.rs:61` defines `AgentId` via the `uuid_id!` macro.
- Workspace `Cargo.toml` lists member crates; all use `edition.workspace = true`.

## Observable contract

- `AgentId` type alias re-exported from `opencode-rk-contracts`.
- `Agent` struct: `id: AgentId`, `name: String`, `capabilities: Vec<String>`,
  `config: AgentConfig`.
- `AgentConfig` struct: `model: String`, `api_key: String`, `base_url: String`.
- `AgentManager` struct: `agents: HashMap<String, Agent>` with methods:
  - `register(&mut self, agent: Agent) -> Result<(), AgentError>`
  - `get(&self, id: &str) -> Option<&Agent>`
  - `list(&self) -> Vec<&Agent>`
  - `remove(&mut self, id: &str) -> bool`
- `AgentError` enum: `Duplicate(String)`, `NotFound(String)`.

## Failure states

- `register` on an existing id returns `AgentError::Duplicate`.
- `get` on a missing id returns `None`.
- `remove` on a missing id returns `false`.

## Resource bounds

- In-memory only; no persistent storage or external I/O.
- `list` returns borrowed references; caller cannot retain ownership.

## Test obligations

- AGENT-001-T01: `register_and_get` - register an agent, retrieve it by id,
  confirm name matches.
- AGENT-001-T02: `list_returns_all` - register two agents, `list` returns both.
- AGENT-001-T03: `remove_deletes` - register an agent, `remove` returns true,
  subsequent `get` returns `None`.
- AGENT-001-T04: `get_unknown_returns_none` - `get` on an empty manager returns
  `None`.
- AGENT-001-T05: `register_duplicate` - registering twice with same id returns
  `AgentError::Duplicate`.

## Verification

- `cargo test -p opencode-rk-agents`
- `cargo check --workspace`
