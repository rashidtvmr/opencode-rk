# Subagent Memory File - Task Registry

## How to use this file

Each subagent reads this file to understand its assigned task.
The orchestrator assigns tasks by task ID. Each task has:
- What crate/file to work in (OWNED FILE - only touch this)
- What to implement
- What tests to write (RED first, then GREEN)
- Verification command
- Acceptance criteria

## Project structure

```
crates/
  contracts/src/lib.rs    - shared types (SessionId, MessageId, etc.)
  foundation/src/lib.rs   - runtime primitives (OwnedTaskScope, ByteBudget, etc.)
  foundation/src/config.rs - configuration system
  storage/src/            - 13 format-2 modules (schema, writer, catalog, etc.)
  security/src/lib.rs     - permission broker, destructive cmd classification
  sessions/src/lib.rs     - session management (MINIMAL - needs implementation)
  server/src/lib.rs       - server daemon (MINIMAL - needs implementation)
  catalog/src/lib.rs      - provider catalog (MINIMAL - needs implementation)
  cli/src/main.rs         - CLI entry point (MINIMAL - needs implementation)
```

## Code style rules

- Compressed single-line style (match existing code)
- No cargo fmt / rustfmt
- No emojis, no em dashes
- No new dependencies without justification
- `#![forbid(unsafe_code)]` in all crates
- All public items get `#[must_use]` where appropriate

## Accepted tasks (DONE - do not touch)

- AUTO-001, AUTO-002, BASE-003, BASE-006, DISC-001, DISC-008, DISC-010, SEC-001, SEC-002

---

## TASK: BASE-001 - Server control plane event bus

**Owned file:** `crates/server/src/event_bus.rs` (NEW)
**Also edit:** `crates/server/src/lib.rs` (add `pub mod event_bus;`)
**Crate:** opencode-rk-server
**Requirement:** Server control plane (opencode.server-control-plane surface)

**What to implement:**
A bounded async event bus for the server daemon. Events flow between components
(session created, message appended, tool executed, permission requested).

```rust
pub struct EventBus { /* bounded channel, subscriber list */ }
pub enum ServerEvent { SessionCreated(SessionId), MessageAppended { session: SessionId, seq: u64 }, ToolExecuted { name: String, duration_ms: u64 }, PermissionRequested(OperationIntent), Shutdown }
pub struct Subscription { /* receiver handle, filter */ }
```

- EventBus::new(capacity: usize) - bounded
- EventBus::publish(event) -> Result (fails if full, never blocks unbounded)
- EventBus::subscribe(filter) -> Subscription
- Subscription::recv() -> Event (async)
- EventBus::subscriber_count() -> usize

**Tests (write RED first):**
1. publish_and_receive - publish event, subscriber receives it
2. bounded_rejects_overflow - fill to capacity, next publish returns error
3. multiple_subscribers - 3 subscribers each get the event
4. filtered_subscription - subscriber with filter only gets matching events
5. drop_subscription_cleanup - dropping Subscription removes it from bus

**Verify:** `cargo test -p opencode-rk-server && cargo check --workspace`
**Acceptance:** all 5 tests green, workspace compiles

---

## TASK: BASE-002 - Wire domain contracts and event schema

**Owned file:** `crates/contracts/src/events.rs` (NEW)
**Also edit:** `crates/contracts/src/lib.rs` (add `pub mod events;`)
**Crate:** opencode-rk-contracts
**Requirement:** Wire domain and event contracts (opencode.contracts-schema surface)

**What to implement:**
Typed event envelope for all domain events. Every event has an ID, timestamp,
source, and typed payload. Used by event bus, storage, and audit trail.

```rust
pub struct EventEnvelope<T> { pub id: EventId, pub timestamp: DateTime<Utc>, pub source: EventSource, pub payload: T }
pub struct EventId(pub String);
pub enum EventSource { System, Session(SessionId), Agent(AgentId), User }
// Domain events:
pub enum DomainEvent { Session(SessionEvent), Message(MessageEvent), Tool(ToolEvent), Permission(PermissionEvent) }
pub enum SessionEvent { Created { id: SessionId, title: String }, Renamed { id: SessionId, title: String }, Archived(SessionId) }
pub enum MessageEvent { Appended { session: SessionId, seq: u64, role: Role }, Compacted { session: SessionId, before: u64, after: u64 } }
pub enum ToolEvent { Invoked { name: String, session: SessionId }, Completed { name: String, duration_ms: u64, success: bool } }
pub enum PermissionEvent { Requested { intent: String }, Granted { intent: String }, Denied { intent: String } }
```

**Tests:**
1. envelope_serde_roundtrip - serialize/deserialize EventEnvelope
2. domain_event_variants - each DomainEvent variant constructs and pattern-matches
3. event_id_uniqueness - 1000 EventId::new() are all distinct
4. source_display - EventSource Display impl works
5. event_ordering - events ordered by timestamp

**Verify:** `cargo test -p opencode-rk-contracts && cargo check --workspace`

---

## TASK: BASE-004 - Singleton daemon with PID lock

**Owned file:** `crates/server/src/daemon.rs` (NEW)
**Also edit:** `crates/server/src/lib.rs` (add `pub mod daemon;`)
**Crate:** opencode-rk-server
**Requirement:** REQ-015 Singleton supports many application clients

**What to implement:**
PID-file lock ensuring one daemon per workspace. Unix socket listener
accepting multiple concurrent clients.

```rust
pub struct SingletonDaemon { /* pid_lock, socket_path, listener, token */ }
pub struct PidLock { /* path, held */ }
pub struct ClientConnection { /* stream, id */ }
```

- PidLock::acquire(path) -> Result<PidLock> (write PID, fail if held by live process)
- PidLock::is_held(path) -> bool (check if another process holds it)
- Drop for PidLock removes file
- SingletonDaemon::bind(socket_path, pid_path) -> Result
- SingletonDaemon::accept_clients() - async loop, tracks connections
- SingletonDaemon::shutdown() - graceful via CancellationToken
- SingletonDaemon::client_count() -> usize

**Tests:**
1. pid_lock_acquire_release - acquire, check held, drop, check not held
2. pid_lock_double_acquire_fails - second acquire returns error
3. pid_lock_stale_reclaim - write fake PID of dead process, acquire succeeds
4. daemon_accept_client - bind, connect, verify client_count=1
5. daemon_shutdown_closes - shutdown, verify accept loop ends

**Verify:** `cargo test -p opencode-rk-server && cargo check --workspace`

---

## TASK: BASE-005 - Multi-client connection handler

**Owned file:** `crates/server/src/clients.rs` (NEW)
**Also edit:** `crates/server/src/lib.rs` (add `pub mod clients;`)
**Crate:** opencode-rk-server
**Requirement:** REQ-015 Singleton supports many application clients

**What to implement:**
Client registry tracking connected clients with IDs, send/receive, and
graceful disconnection.

```rust
pub struct ClientRegistry { /* map of ClientId -> ClientHandle */ }
pub struct ClientHandle { /* sender, metadata */ }
pub struct ClientId(pub u64);
pub struct ClientMessage { pub client: ClientId, pub payload: Vec<u8> }
```

- ClientRegistry::new(max_clients) -> bounded
- register(stream) -> ClientId
- unregister(ClientId)
- broadcast(msg) -> sends to all
- send_to(ClientId, msg) -> sends to one
- connected_count() -> usize

**Tests:**
1. register_and_count - register 3, count=3
2. unregister_removes - register, unregister, count=0
3. max_clients_enforced - exceed limit returns error
4. broadcast_reaches_all - broadcast, all receive
5. send_to_specific - send to one, only that one receives

**Verify:** `cargo test -p opencode-rk-server && cargo check --workspace`

---

## TASK: BASE-007 - Runtime lifecycle manager

**Owned file:** `crates/foundation/src/lifecycle.rs` (NEW)
**Also edit:** `crates/foundation/src/lib.rs` (add `pub mod lifecycle;`)
**Crate:** opencode-rk-foundation
**Requirement:** Configuration and runtime lifecycle (opencode.configuration-runtime surface)

**What to implement:**
Application lifecycle phases with ordered startup/shutdown hooks.

```rust
pub enum Phase { Init, Starting, Running, Stopping, Stopped }
pub struct LifecycleManager { /* phase, hooks */ }
pub struct HookId(pub u64);
pub type HookFn = Box<dyn FnOnce() -> Result<(), Box<dyn std::error::Error>> + Send>;
```

- LifecycleManager::new() starts in Init
- register_startup_hook(priority, hook) -> HookId
- register_shutdown_hook(priority, hook) -> HookId
- start() - runs startup hooks in priority order, transitions Init->Starting->Running
- stop() - runs shutdown hooks in reverse priority, transitions Running->Stopping->Stopped
- phase() -> Phase
- Cannot register hooks after Running phase

**Tests:**
1. phase_transitions - init->starting->running->stopping->stopped
2. startup_hooks_ordered - hooks run in priority order
3. shutdown_hooks_reverse - hooks run in reverse priority
4. no_hooks_after_running - register after start returns error
5. double_start_fails - calling start twice returns error

**Verify:** `cargo test -p opencode-rk-foundation && cargo check --workspace`

---

## TASK: BASE-008 - Effect runtime adapter

**Owned file:** `crates/foundation/src/effect.rs` (NEW)
**Also edit:** `crates/foundation/src/lib.rs` (add `pub mod effect;`)
**Crate:** opencode-rk-foundation
**Requirement:** Effect runtime and SQLite adapter (opencode.effect-runtime surface)

**What to implement:**
Async effect execution with retry, timeout, and result capture. Effects are
units of work (DB query, HTTP call, file op) with uniform error handling.

```rust
pub enum Effect { Db(DbEffect), Io(IoEffect), Net(NetEffect) }
pub enum DbEffect { Query(String), Execute(String) }
pub enum IoEffect { ReadFile(PathBuf), WriteFile(PathBuf, Vec<u8>) }
pub enum NetEffect { HttpGet(String), HttpPost(String, Vec<u8>) }
pub struct EffectResult { pub effect: Effect, pub outcome: Outcome, pub duration: Duration }
pub enum Outcome { Success(Vec<u8>), Failure(String), Timeout }
pub struct EffectRunner { /* timeout, retry config */ }
```

- EffectRunner::new(default_timeout, max_retries)
- run(effect) -> EffectResult (with timeout and retry)
- run_batch(effects) -> Vec<EffectResult> (parallel)
- dry_run(effect) -> validates effect without executing

**Tests:**
1. io_read_write_roundtrip - write file, read back, match content
2. timeout_returns_timeout_outcome - slow effect times out
3. dry_run_validates - dry run succeeds for valid, fails for invalid path
4. batch_parallel - 3 effects run and return 3 results
5. retry_on_transient - mock transient failure, retry succeeds

**Verify:** `cargo test -p opencode-rk-foundation && cargo check --workspace`

---

## TASK: DB-001 - Storage facade (public API)

**Owned file:** `crates/storage/src/facade.rs` (NEW)
**Also edit:** `crates/storage/src/lib.rs` (add `pub mod facade;`)
**Crate:** opencode-rk-storage
**Requirement:** REQ-033 Embedded scalable storage without Postgres

**What to implement:**
Single entry-point struct that initializes the DB, runs v2 migrations, and
exposes writer/catalog APIs.

```rust
pub struct StorageFacade { /* connection, blob_store */ }
```

- StorageFacade::open(path) -> Result - creates DB, runs schema_v2 init
- StorageFacade::open_memory() -> Result - in-memory for testing
- writer() -> &V2Writer
- catalog() -> &CatalogV2
- close() - graceful shutdown

**Tests:**
1. open_and_init - open, verify tables exist
2. write_and_read - create session, write message, read back
3. reopen_persists - write, close, reopen, data still there
4. open_memory_works - in-memory mode works
5. concurrent_access - two threads write simultaneously without corruption

**Verify:** `cargo test -p opencode-rk-storage && cargo check --workspace`

---

## TASK: DB-003 - Session management

**Owned file:** `crates/sessions/src/lib.rs` (REPLACE - currently 7 lines)
**Crate:** opencode-rk-sessions
**Requirement:** REQ-006 Session management, REQ-014 Subagent context in DB

**What to implement:**
Full session CRUD using storage v2 APIs.

```rust
pub struct SessionManager { /* connection */ }
```

- SessionManager::new(conn) -> Self
- create_session(title) -> SessionId
- get_session(id) -> Option<SessionRecord>
- list_sessions(limit, offset) -> Vec<SessionSummary>
- rename_session(id, new_title) -> Result
- archive_session(id) -> Result
- append_message(session_id, role, content) -> MessageId
- list_messages(session_id, limit) -> Vec<MessageRecord>
- session_message_count(session_id) -> u64

Use storage crate's V2Writer and schema_v2 directly. Read
crates/storage/src/writer_v2.rs and crates/contracts/src/lib.rs for types.

**Tests:**
1. create_and_get - create session, get by id, verify title
2. list_sessions_ordered - create 3, list returns newest first
3. rename_works - create, rename, get shows new title
4. archive_hides - create, archive, list doesn't show it
5. append_and_list_messages - append 5 messages, list returns all 5 in order

**Verify:** `cargo test -p opencode-rk-sessions && cargo check --workspace`

---

## TASK: SEC-003 - Trusted user toggle

**Owned file:** `crates/security/src/trusted.rs` (NEW)
**Also edit:** `crates/security/src/lib.rs` (add `pub mod trusted;`)
**Crate:** opencode-rk-security
**Requirement:** REQ-021, REQ-029, REQ-030

**What to implement:**
Trusted user mode that relaxes some protections but NEVER bypasses mandatory
controls (star-proof from SEC-001).

```rust
pub struct TrustLevel { /* level, granted_at, expires_at */ }
pub enum Trust { Untrusted, SessionTrust, FullTrust }
pub struct TrustedUserToggle { /* current level, audit */ }
```

- TrustedUserToggle::new() - starts Untrusted
- grant(level, duration) - grants trust for duration
- revoke() - immediately revokes
- current() -> Trust
- is_expired() -> bool
- can_bypass(operation) -> bool - returns true ONLY for non-mandatory ops
- Mandatory controls (is_secret_path, classify_destructive_argv returning Deny) NEVER bypassed

**Tests:**
1. default_untrusted - new toggle is Untrusted
2. grant_and_check - grant SessionTrust, verify current()
3. expiry_works - grant with 0ms duration, check is_expired
4. mandatory_never_bypassed - FullTrust still blocks .env access
5. revoke_immediate - grant, revoke, verify Untrusted

**Verify:** `cargo test -p opencode-rk-security && cargo check --workspace`

---

## TASK: SEC-004 - Sensitive file and env protection

**Owned file:** `crates/security/src/sensitive.rs` (NEW)
**Also edit:** `crates/security/src/lib.rs` (add `pub mod sensitive;`)
**Crate:** opencode-rk-security
**Requirement:** REQ-027 OS-enforced .env and sensitive-file restrictions

**What to implement:**
Extended sensitive path detection + environment variable filtering.

```rust
pub struct SensitivePathChecker { /* patterns */ }
pub struct EnvFilter { /* deny patterns */ }
```

- SensitivePathChecker covers: .env*, .ssh/, .aws/, .config/gcloud, .gnupg/,
  id_rsa, id_ed25519, .netrc, .npmrc, .docker/config.json, .kube/config,
  .git-credentials, **/token*, **/credentials*
- EnvFilter::filter(env_map) -> filtered_map - removes vars matching:
  *_KEY, *_SECRET, *_TOKEN, *_PASSWORD, *_CREDENTIAL, AWS_*, OPENAI_*,
  ANTHROPIC_*, GITHUB_TOKEN, DATABASE_URL (if contains password)
- Both are configurable (add/remove patterns)

**Tests:**
1. blocks_dotenv - .env, .env.local, .env.production all blocked
2. blocks_ssh_keys - .ssh/id_rsa, .ssh/id_ed25519 blocked
3. blocks_cloud_creds - .aws/credentials, .config/gcloud blocked
4. env_filter_removes_secrets - API_KEY, AWS_SECRET removed
5. env_filter_keeps_safe - PATH, HOME, TERM preserved

**Verify:** `cargo test -p opencode-rk-security && cargo check --workspace`

---

## TASK: SEC-005 - Landlock sandbox enforcement

**Owned file:** `crates/security/src/sandbox.rs` (NEW)
**Also edit:** `crates/security/src/lib.rs` (add `pub mod sandbox;`)
**Crate:** opencode-rk-security
**Requirement:** REQ-027

**What to implement:**
Filesystem sandbox configuration using Landlock (Linux kernel). NOT the actual
syscall (that requires unsafe) - just the policy struct and path resolution.

```rust
pub struct SandboxPolicy { pub allowed_read: Vec<PathBuf>, pub allowed_write: Vec<PathBuf>, pub denied: Vec<PathBuf> }
pub struct SandboxCheck { /* policy */ }
```

- SandboxPolicy::default() - sensible defaults (cwd read/write, home read, /tmp write)
- SandboxCheck::is_allowed(path, action) -> Result<(), SandboxDenied>
- Denied paths always take precedence over allowed
- Path traversal (..) is resolved before checking
- Symlinks resolved to real path before checking

**Tests:**
1. default_allows_cwd - cwd read/write allowed
2. denied_overrides_allowed - /etc in allowed_read, /etc/shadow in denied -> denied wins
3. traversal_blocked - ../../../etc/passwd resolved and blocked
4. symlink_resolved - symlink to denied path is denied
5. outside_sandbox_denied - path outside all allowed dirs is denied

**Verify:** `cargo test -p opencode-rk-security && cargo check --workspace`

---

## TASK: SEC-006 - System file read-only enforcement

**Owned file:** `crates/security/src/sysfiles.rs` (NEW)
**Also edit:** `crates/security/src/lib.rs` (add `pub mod sysfiles;`)
**Crate:** opencode-rk-security
**Requirement:** REQ-028, REQ-029

**What to implement:**
System files readable but not writable. Trusted user can read more but still
cannot write system files.

```rust
pub struct SystemFilePolicy { /* read_allowed, write_denied */ }
```

- SYSTEM_READ_PATHS: /etc/hosts, /etc/resolv.conf, /proc/cpuinfo, /etc/os-release
- SYSTEM_WRITE_DENIED: /etc/**, /usr/**, /boot/**, /sbin/**, /sys/**
- check_read(path) -> Result
- check_write(path) -> Result<(), SystemFileDenied> (always denied for system paths)
- Trusted user: can read /etc/** but still cannot write

**Tests:**
1. read_etc_hosts_allowed - /etc/hosts readable
2. write_etc_hosts_denied - writing /etc/hosts denied
3. trusted_read_more - trusted can read /etc/passwd
4. trusted_write_still_denied - trusted still cannot write /etc/anything
5. non_system_allowed - /home/user/file.txt read and write both OK

**Verify:** `cargo test -p opencode-rk-security && cargo check --workspace`

---

## TASK: SEC-007 - Cross-project file access control

**Owned file:** `crates/security/src/project_boundary.rs` (NEW)
**Also edit:** `crates/security/src/lib.rs` (add `pub mod project_boundary;`)
**Crate:** opencode-rk-security
**Requirement:** REQ-031

**What to implement:**
Files outside the project root require explicit approval.

```rust
pub struct ProjectBoundary { pub root: PathBuf, pub approved_external: Vec<PathBuf> }
```

- ProjectBoundary::new(root)
- is_within_project(path) -> bool
- check_access(path) -> Result<(), NeedsApproval>
- approve_external(path) - adds to approved list
- revoke_external(path) - removes from approved list

**Tests:**
1. within_project_allowed - file in project dir allowed
2. outside_project_needs_approval - file in /tmp needs approval
3. approved_external_works - approve /tmp/data.json, then access allowed
4. revoke_removes_access - approve then revoke, access denied again
5. parent_traversal_detected - ../other-project/file detected as outside

**Verify:** `cargo test -p opencode-rk-security && cargo check --workspace`

---

## TASK: SEC-010 - Pre-tool hook bus

**Owned file:** `crates/security/src/hooks.rs` (NEW)
**Also edit:** `crates/security/src/lib.rs` (add `pub mod hooks;`)
**Crate:** opencode-rk-security
**Requirement:** REQ-020

**What to implement:**
Bounded hook system: before a tool executes, pre-hooks can inspect/block.
After execution, post-hooks observe results. Bus bounded to 100 hooks max.

```rust
pub struct HookBus { /* pre_hooks, post_hooks, capacity=100 */ }
pub enum HookDecision { Allow, Deny(String), Modify(String) }
pub struct PreHook { pub id: HookId, pub filter: String, pub handler: Box<dyn Fn(&ToolInvocation) -> HookDecision + Send + Sync> }
pub struct PostHook { pub id: HookId, pub handler: Box<dyn Fn(&ToolResult) + Send + Sync> }
pub struct ToolInvocation { pub name: String, pub args: serde_json::Value }
pub struct ToolResult { pub name: String, pub success: bool, pub duration_ms: u64 }
```

- HookBus::new(capacity) - bounded to capacity (max 100)
- register_pre(hook) -> Result<HookId> (fails if at capacity)
- register_post(hook) -> Result<HookId>
- run_pre_hooks(invocation) -> HookDecision (first Deny wins)
- run_post_hooks(result)
- unregister(HookId)

**Tests:**
1. pre_hook_can_block - register deny hook, verify tool blocked
2. post_hook_observes - register post hook, verify called after tool
3. bounded_to_capacity - register 100, 101st fails
4. first_deny_wins - 3 hooks: allow, deny, allow -> deny
5. unregister_removes - register, unregister, hook not called

**Verify:** `cargo test -p opencode-rk-security && cargo check --workspace`

---

## TASK: SEC-011 - Post-tool hook and SSRF guard

**Owned file:** `crates/security/src/ssrf.rs` (NEW)
**Also edit:** `crates/security/src/lib.rs` (add `pub mod ssrf;`)
**Crate:** opencode-rk-security
**Requirement:** REQ-020

**What to implement:**
SSRF prevention: block tool calls that would hit internal/private IPs.

```rust
pub struct SsrfGuard { /* blocked_ranges */ }
```

- SsrfGuard::new() with default blocked ranges:
  127.0.0.0/8, 10.0.0.0/8, 172.16.0.0/12, 192.168.0.0/16,
  169.254.0.0/16 (link-local), ::1, fd00::/8, fe80::/10
- check_url(url) -> Result<(), SsrfBlocked>
- check_ip(ip) -> Result<(), SsrfBlocked>
- allow_localhost(bool) - toggle for dev mode
- Custom blocked ranges can be added

**Tests:**
1. blocks_localhost - 127.0.0.1, localhost blocked
2. blocks_private_ranges - 10.x, 172.16.x, 192.168.x blocked
3. allows_public - 8.8.8.8, google.com allowed
4. blocks_ipv6_loopback - ::1 blocked
5. dev_mode_allows_localhost - with allow_localhost(true), 127.0.0.1 allowed

**Verify:** `cargo test -p opencode-rk-security && cargo check --workspace`

---

## TASK: DB-006 - Storage migration runner

**Owned file:** `crates/storage/src/migrations.rs` (NEW)
**Also edit:** `crates/storage/src/lib.rs` (add `pub mod migrations;`)
**Crate:** opencode-rk-storage
**Requirement:** REQ-033

**What to implement:**
Run schema migrations in order, track applied migrations, skip already-applied.

```rust
pub struct MigrationRunner { /* conn */ }
pub struct Migration { pub version: u32, pub name: String, pub sql: String, pub checksum: String }
pub struct AppliedMigration { pub version: u32, pub applied_at: DateTime<Utc>, pub checksum: String }
```

- MigrationRunner::new(conn)
- run_pending() -> Result<Vec<u32>> (returns applied versions)
- applied() -> Vec<AppliedMigration>
- verify_checksums() -> Result (detects tampered migrations)
- Creates `_migrations` table if not exists

**Tests:**
1. empty_db_runs_all - fresh DB, all migrations run
2. skip_already_applied - run twice, second time skips
3. checksum_mismatch_fails - tamper migration, verify detects
4. ordered_execution - migrations run in version order
5. partial_failure_rolls_back - bad SQL in migration 3, migrations 1-2 preserved

**Verify:** `cargo test -p opencode-rk-storage && cargo check --workspace`
