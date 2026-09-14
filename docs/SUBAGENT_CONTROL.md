# Subagent lifecycle control - complete specification

This document specifies every subagent control surface the lean harness exposes.
It covers spawn modes, failure recovery, runtime model switching, context management,
settings, observability, permissions, output handling, and cleanup. Each section
maps to one or more ralph.json stories and test obligations.

## 1. Spawn modes

### 1.1 Context fork (AGENT-016)

Clone the parent session's conversation state into a new child agent. The fork
includes selected context (not the entire history) to stay within token limits.

```rust
pub struct ForkOptions {
    pub parent_session_id: SessionId,
    pub include_system_prompt: bool,
    pub include_last_n_messages: Option<usize>,
    pub include_tool_results: bool,
    pub additional_context: Vec<ContextBlock>,
    pub provider: Option<ProviderId>,
    pub model: Option<ModelId>,
}

pub struct ContextBlock {
    pub role: Role,
    pub content: String,
    pub metadata: Option<serde_json::Value>,
}
```

Behavior:
- Parent session state is read-only during fork (snapshot isolation).
- Child gets a new SessionId with `parent_id` backlink.
- Context is trimmed to fit the child model's context window.
- Child inherits parent's permission grants unless explicitly restricted.
- Fork is atomic: either the child is fully created or nothing changes.

### 1.2 Fresh context (AGENT-017)

Launch a child agent with zero inherited state. All context must be explicitly
provided in the spawn call.

```rust
pub struct SpawnOptions {
    pub context_bundle: ContextBundle,
    pub provider: ProviderId,
    pub model: ModelId,
    pub working_directory: PathBuf,
    pub timeout: Duration,
    pub max_tokens: u64,
    pub budget: Budget,
}

pub struct ContextBundle {
    pub system_prompt: String,
    pub files: Vec<FileAttachment>,
    pub instructions: Vec<String>,
    pub environment: HashMap<String, String>,
}
```

Behavior:
- No state leakage from parent. Environment is filtered through SensitiveEnvironmentFilter.
- Files are validated (exist, readable, within size budget) before spawn.
- The child's context window starts clean with only the provided bundle.

### 1.3 Multi-provider routing (AGENT-018)

Each child agent can use a different provider and model than its parent or siblings.

```rust
pub struct SubagentRouter {
    pub default_provider: ProviderId,
    pub default_model: ModelId,
    pub fallback_chain: Vec<(ProviderId, ModelId)>,
    pub task_routing: HashMap<TaskCategory, (ProviderId, ModelId)>,
}

pub enum TaskCategory {
    CodeGeneration,
    CodeReview,
    Research,
    Testing,
    Documentation,
    Refactoring,
}
```

Behavior:
- Router selects provider/model based on task category or explicit override.
- Fallback chain is tried in order when primary fails (transient error only).
- Per-child model override in SpawnOptions/ForkOptions takes precedence over router.
- Provider credentials are resolved from the parent's configuration, never hardcoded.

## 2. Failure recovery

### 2.1 Resume failed subagent (AGENT-019)

Restore a failed child's session from the last checkpoint and continue execution
from the failure point.

```rust
pub struct ResumeOptions {
    pub session_id: SessionId,
    pub from_checkpoint: Option<CheckpointId>,
    pub preserve_partial_results: bool,
    pub new_model: Option<ModelId>,
    pub additional_context: Vec<ContextBlock>,
}

pub struct Checkpoint {
    pub id: CheckpointId,
    pub session_id: SessionId,
    pub message_seq: u64,
    pub timestamp: DateTime<Utc>,
    pub tool_state: Option<serde_json::Value>,
    pub token_usage: TokenUsage,
}
```

Behavior:
- Checkpoints are created automatically after each successful tool call.
- Resume replays the conversation up to the checkpoint, then continues.
- Partial results (files written, tests added) are preserved unless the caller
  opts out.
- If the failure was a model error, resume can switch to a different model.

### 2.2 Retry with exponential backoff (AGENT-020)

Classify errors and retry transient failures with jitter and circuit breaker.

```rust
pub struct RetryPolicy {
    pub max_attempts: u32,
    pub initial_delay: Duration,
    pub max_delay: Duration,
    pub multiplier: f64,
    pub jitter: JitterStrategy,
    pub circuit_breaker: CircuitBreakerConfig,
}

pub enum JitterStrategy {
    None,
    Full,
    Equal,
    Decorrelated,
}

pub struct CircuitBreakerConfig {
    pub failure_threshold: u32,
    pub recovery_timeout: Duration,
    pub half_open_max_calls: u32,
}

pub enum ErrorClass {
    Transient(TransientReason),
    Permanent(PermanentReason),
    Unknown,
}

pub enum TransientReason {
    RateLimit,
    ServerOverload,
    NetworkTimeout,
    ConcurrencyLimit,
    ModelBusy,
}

pub enum PermanentReason {
    InvalidRequest,
    AuthenticationFailed,
    ModelNotFound,
    ContentFiltered,
    BudgetExhausted,
    ContextOverflow,
}
```

Behavior:
- Every provider error is classified into ErrorClass before retry decision.
- Transient errors are retried; permanent errors fail immediately.
- Exponential backoff: delay = min(initial * multiplier^attempt + jitter, max_delay).
- Circuit breaker opens after failure_threshold consecutive failures, preventing
  further calls for recovery_timeout, then allows half_open_max_calls to test.
- Retries preserve the session state; each retry continues from the last checkpoint.

### 2.3 Change model mid-session (AGENT-021)

Hot-swap the provider or model for a running subagent without losing conversation
context.

```rust
pub struct ModelSwitchRequest {
    pub session_id: SessionId,
    pub new_provider: Option<ProviderId>,
    pub new_model: ModelId,
    pub reason: SwitchReason,
    pub context_adaptation: ContextAdaptation,
}

pub enum SwitchReason {
    ErrorFallback,
    CostOptimization,
    CapabilityUpgrade,
    ManualOverride,
}

pub enum ContextAdaptation {
    TruncateToFit,
    CompressToFit,
    RejectIfOverflow,
}
```

Behavior:
- The current conversation is checkpointed before the switch.
- If the new model's context window is smaller, ContextAdaptation determines
  how to handle overflow (truncate oldest, compress, or reject the switch).
- The model switch is recorded in the session's audit trail.
- System prompts are re-evaluated for model-specific adaptations.
- Tool availability may change based on the new model's capabilities.

## 3. Context management

### 3.1 Subagent context compression (AGENT-022)

Automatic context compaction when approaching the model's token limit.

```rust
pub struct CompressionConfig {
    pub strategy: CompressionStrategy,
    pub threshold_ratio: f64,
    pub min_preserved_messages: usize,
    pub preserve_system_prompt: bool,
    pub preserve_last_n_turns: usize,
    pub quality_target: CompressionQuality,
}

pub enum CompressionStrategy {
    SlidingWindow { window_size: usize },
    Summarize { model: Option<ModelId> },
    DropOldest,
    Hybrid { summarize_before: usize, keep_recent: usize },
}

pub enum CompressionQuality {
    Fast,
    Balanced,
    High,
}

pub struct CompressionMetrics {
    pub original_tokens: u64,
    pub compressed_tokens: u64,
    pub ratio: f64,
    pub messages_removed: usize,
    pub messages_summarized: usize,
    pub timestamp: DateTime<Utc>,
}
```

Behavior:
- Compression triggers when token usage exceeds threshold_ratio of context window.
- System prompt and last N turns are always preserved.
- Summarize strategy uses a cheap model (configurable) to create summaries.
- Hybrid combines summarization for old context with full retention for recent.
- Metrics are recorded for observability.
- Compression never discards tool call results from the current task.

### 3.2 Structured handoff protocol (AGENT-023)

Serialize a subagent's state into a portable handoff document that another agent
can consume with fidelity guarantees.

```rust
pub struct HandoffDocument {
    pub version: u32,
    pub source_session: SessionId,
    pub source_model: ModelId,
    pub timestamp: DateTime<Utc>,
    pub objective: String,
    pub completed_steps: Vec<CompletedStep>,
    pub pending_work: Vec<PendingItem>,
    pub key_decisions: Vec<Decision>,
    pub file_changes: Vec<FileChange>,
    pub context_summary: String,
    pub warnings: Vec<String>,
    pub checksum: String,
}

pub struct CompletedStep {
    pub description: String,
    pub evidence: Vec<String>,
    pub files_modified: Vec<PathBuf>,
}

pub struct PendingItem {
    pub description: String,
    pub priority: Priority,
    pub blockers: Vec<String>,
}
```

Behavior:
- Handoff is requested explicitly or triggered automatically on session end.
- The document is self-contained: a new agent can continue without the original
  session's message history.
- Checksum covers the document content for integrity verification.
- File changes include before/after hashes for conflict detection.
- The receiving agent validates the handoff document before accepting.

### 3.3 Auto context compression toggle (AGENT-024)

Settings-driven automatic compression with configurable thresholds.

```rust
pub struct AutoCompressionSettings {
    pub enabled: bool,
    pub trigger_threshold: f64,
    pub strategy: CompressionStrategy,
    pub quality: CompressionQuality,
    pub notify_on_compress: bool,
    pub max_compressions_per_session: Option<u32>,
    pub min_interval_between_compressions: Duration,
}
```

Behavior:
- When enabled, compression runs automatically without user intervention.
- Notifications can be shown/suppressed per setting.
- Rate limiting prevents excessive compression (min interval, max count).
- Can be toggled at runtime via the settings API or config file.
- Disabled by default; explicit opt-in required.

## 4. Settings and configuration

### 4.1 Dedicated subagent settings (AGENT-025)

A dedicated configuration section for all subagent behavior, accessible via the
settings UI and config file.

```rust
pub struct SubagentSettings {
    pub max_concurrent: u32,
    pub default_provider: ProviderId,
    pub default_model: ModelId,
    pub fallback_models: Vec<ModelId>,
    pub retry_policy: RetryPolicy,
    pub compression: AutoCompressionSettings,
    pub timeout: Duration,
    pub budget: SubagentBudget,
    pub permissions: SubagentPermissionPolicy,
    pub observability: ObservabilitySettings,
    pub pool: PoolSettings,
    pub handoff: HandoffSettings,
}

pub struct SubagentBudget {
    pub max_tokens_per_child: u64,
    pub max_cost_per_child_usd: f64,
    pub max_total_cost_usd: f64,
    pub budget_mode: BudgetMode,
}

pub enum BudgetMode {
    FreeWorkersOnly,
    BudgetCapped,
    Unlimited,
}

pub struct SubagentPermissionPolicy {
    pub inherit_parent: bool,
    pub allowed_tools: Option<Vec<String>>,
    pub denied_tools: Option<Vec<String>>,
    pub max_file_write_size: u64,
    pub allowed_paths: Vec<PathBuf>,
    pub denied_paths: Vec<PathBuf>,
}

pub struct ObservabilitySettings {
    pub live_status: bool,
    pub token_tracking: bool,
    pub cost_tracking: bool,
    pub latency_tracking: bool,
    pub error_rate_tracking: bool,
}

pub struct PoolSettings {
    pub min_pool_size: u32,
    pub max_pool_size: u32,
    pub idle_timeout: Duration,
    pub scale_strategy: ScaleStrategy,
}

pub enum ScaleStrategy {
    Fixed,
    WorkQueueBased { tasks_per_agent: u32 },
    Adaptive { target_utilization: f64 },
}

pub struct HandoffSettings {
    pub auto_handoff_on_failure: bool,
    pub auto_handoff_on_timeout: bool,
    pub handoff_format: HandoffFormat,
}

pub enum HandoffFormat {
    Markdown,
    Json,
    Both,
}
```

Settings are layered (defaults, user config, project config, env vars, CLI overrides)
using the config system from BASE-006.

## 5. Pool management

### 5.1 Subagent pool manager (AGENT-026)

Maintain a pool of running agents, auto-replacing finished or failed ones,
draining a work queue, with pool scaling.

```rust
pub struct SubagentPool {
    pool_id: PoolId,
    config: PoolSettings,
    workers: Vec<PoolWorker>,
    queue: VecDeque<QueuedTask>,
    metrics: PoolMetrics,
}

pub struct PoolWorker {
    pub id: WorkerId,
    pub session_id: SessionId,
    pub status: WorkerStatus,
    pub current_task: Option<TaskId>,
    pub provider: ProviderId,
    pub model: ModelId,
    pub started_at: DateTime<Utc>,
    pub tasks_completed: u32,
    pub total_tokens: u64,
}

pub enum WorkerStatus {
    Idle,
    Running,
    Failed { error: String, attempts: u32 },
    Draining,
    Stopped,
}

pub struct QueuedTask {
    pub id: TaskId,
    pub priority: Priority,
    pub enqueued_at: DateTime<Utc>,
    pub max_attempts: u32,
    pub routing: Option<TaskCategory>,
}

pub struct PoolMetrics {
    pub total_tasks_completed: u64,
    pub total_tasks_failed: u64,
    pub total_tokens_used: u64,
    pub avg_task_duration: Duration,
    pub current_utilization: f64,
    pub queue_depth: usize,
}
```

Behavior:
- Pool maintains min_pool_size workers at all times when work is available.
- Finished workers are replaced immediately with next queued task.
- Failed workers are retried per retry policy, then marked blocked.
- Pool draining: stop accepting new tasks, finish current, then shut down.
- Scale strategy determines how pool size adjusts to queue depth.
- Each worker is isolated (own session, own context, own worktree if needed).

## 6. Observability

### 6.1 Subagent observability (AGENT-027)

Live dashboard for monitoring all child agents.

```rust
pub struct SubagentDashboard {
    pub children: Vec<ChildStatus>,
    pub aggregate: AggregateMetrics,
    pub events: Vec<SubagentEvent>,
}

pub struct ChildStatus {
    pub session_id: SessionId,
    pub task_id: Option<TaskId>,
    pub model: ModelId,
    pub status: WorkerStatus,
    pub token_usage: TokenUsage,
    pub cost_usd: f64,
    pub duration: Duration,
    pub last_activity: DateTime<Utc>,
    pub error_count: u32,
    pub checkpoint_count: u32,
    pub compression_count: u32,
}

pub struct AggregateMetrics {
    pub total_children_spawned: u64,
    pub active_children: u32,
    pub total_tokens: u64,
    pub total_cost_usd: f64,
    pub avg_latency: Duration,
    pub p50_latency: Duration,
    pub p95_latency: Duration,
    pub p99_latency: Duration,
    pub error_rate: f64,
    pub success_rate: f64,
}

pub struct TokenUsage {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cached_tokens: u64,
    pub total_tokens: u64,
}

pub enum SubagentEvent {
    Spawned { session_id: SessionId, model: ModelId },
    Completed { session_id: SessionId, duration: Duration },
    Failed { session_id: SessionId, error: String },
    ModelSwitched { session_id: SessionId, from: ModelId, to: ModelId },
    Compressed { session_id: SessionId, ratio: f64 },
    Checkpointed { session_id: SessionId, checkpoint_id: CheckpointId },
    Resumed { session_id: SessionId, from_checkpoint: CheckpointId },
    Cancelled { session_id: SessionId, reason: String },
    HandedOff { from: SessionId, to: SessionId },
    BudgetWarning { session_id: SessionId, used: f64, limit: f64 },
}
```

Behavior:
- Dashboard is queryable in real-time (TUI shows live table).
- Events are append-only, bounded (last 10000, oldest evicted).
- Latency percentiles computed over sliding 5-minute window.
- Cost tracking aggregated per provider, per model, per task category.
- Error rate computed as errors / (errors + successes) over sliding window.

## 7. Permissions

### 7.1 Subagent permission inheritance (AGENT-028)

Children inherit parent permissions with optional narrowing (never widening).

```rust
pub struct PermissionInheritance {
    pub mode: InheritanceMode,
    pub restrictions: Vec<PermissionRestriction>,
}

pub enum InheritanceMode {
    InheritAll,
    InheritExplicit(Vec<Permission>),
    NoInheritance,
}

pub struct PermissionRestriction {
    pub tool: String,
    pub action: RestrictionAction,
}

pub enum RestrictionAction {
    Deny,
    RequireHuman,
    LimitPaths(Vec<PathBuf>),
    ReadOnly,
}
```

Behavior:
- Default: children inherit all parent permissions.
- Restrictions can narrow (never widen) the inherited set.
- A child cannot gain permissions the parent does not have.
- Mandatory controls (star-proof) propagate unconditionally.
- Permission inheritance is recorded in the child's audit trail.

## 8. Output handling

### 8.1 Subagent output aggregation (AGENT-029)

Collect, merge, and deduplicate results from parallel children.

```rust
pub struct AggregationConfig {
    pub merge_strategy: MergeStrategy,
    pub conflict_resolution: ConflictResolution,
    pub dedup_by: DedupKey,
}

pub enum MergeStrategy {
    FirstWins,
    LastWins,
    MergeAll,
    ManualReview,
}

pub enum ConflictResolution {
    KeepBoth,
    PreferNewer,
    PreferLarger,
    AskHuman,
}

pub enum DedupKey {
    FilePath,
    ContentHash,
    TaskId,
}

pub struct AggregatedResult {
    pub task_id: TaskId,
    pub children: Vec<ChildResult>,
    pub merged_files: Vec<MergedFile>,
    pub conflicts: Vec<FileConflict>,
    pub summary: String,
}

pub struct MergedFile {
    pub path: PathBuf,
    pub source_child: SessionId,
    pub hash: String,
}

pub struct FileConflict {
    pub path: PathBuf,
    pub versions: Vec<(SessionId, String)>,
    pub resolution: Option<ConflictResolution>,
}
```

Behavior:
- Aggregation runs after all children in a batch complete.
- File-level merge detects conflicts (same path, different content).
- Content-hash dedup prevents storing duplicate outputs.
- Unresolved conflicts are surfaced to the parent for manual resolution.
- Summary is generated from all children's results.

## 9. Cleanup

### 9.1 Subagent cancellation and cleanup (AGENT-030)

Cancel running children, reclaim resources, clean up worktrees, release locks.

```rust
pub struct CancellationRequest {
    pub target: CancelTarget,
    pub reason: String,
    pub cleanup: CleanupPolicy,
    pub grace_period: Duration,
}

pub enum CancelTarget {
    Single(SessionId),
    ByTask(TaskId),
    All,
    ByStatus(WorkerStatus),
}

pub enum CleanupPolicy {
    Full,
    KeepWorktree,
    KeepLogs,
    Minimal,
}
```

Behavior:
- Cancellation sends CancellationToken to the child's task scope.
- Grace period allows the child to checkpoint before termination.
- After grace period, force-terminate (SIGTERM then SIGKILL for subprocess).
- Cleanup removes: worktree (if Full), session state (if Full), lock files (always).
- Logs are preserved unless Full cleanup is requested.
- Partial results are preserved per the CleanupPolicy.

## 10. Cross-cutting concerns

### 10.1 Budget enforcement

Every subagent operation checks the budget before proceeding:
- Token budget: per-child and aggregate.
- Cost budget: per-child and aggregate (in USD).
- Time budget: per-child timeout.
- Free-workers-only mode: reject any operation that would incur cost.

### 10.2 Audit trail

Every subagent lifecycle event is recorded in the session's audit trail:
- Spawn (with full options).
- Resume (with checkpoint reference).
- Model switch (with reason).
- Compression (with metrics).
- Cancellation (with reason).
- Budget warning (with thresholds).
- Handoff (with document checksum).

### 10.3 Persistence

All subagent state is persisted to the storage layer (format-2 schema):
- Session records with parent_id backlinks.
- Checkpoint records per session.
- Compression history per session.
- Audit events per session.
- Aggregated metrics per pool.

### 10.4 Thread safety

All subagent operations are safe for concurrent use:
- Pool manager uses Arc<Mutex> for worker list access.
- Queue uses bounded channel (from foundation crate).
- Metrics use atomic counters where possible.
- File operations use advisory locks.

## Appendix A: comparison with existing harnesses

| Feature | Claude Code | Codex | OpenCode | Lean Harness |
|---------|------------|-------|----------|-------------|
| Context fork | Yes (--fork) | Yes (ForkBoundary) | No | AGENT-016 |
| Fresh context | Yes (Task) | Yes (subagent) | Yes (opencode run) | AGENT-017 |
| Multi-provider | No | No | Yes (9router) | AGENT-018 |
| Resume failed | Partial (--continue) | No | Yes (--continue) | AGENT-019 |
| Exponential backoff | Internal only | Internal only | No | AGENT-020 |
| Model switch mid-session | No | No | No | AGENT-021 |
| Context compression | Yes (autoCompact) | No | No | AGENT-022 |
| Structured handoff | No | No | No | AGENT-023 |
| Auto compression | Yes (threshold-based) | No | No | AGENT-024 |
| Subagent settings UI | Partial | Partial | No | AGENT-025 |
| Pool manager | No | No | No | AGENT-026 |
| Observability | Partial (cost tracker) | No | No | AGENT-027 |
| Permission inheritance | Yes (implicit) | Yes (sandbox) | No | AGENT-028 |
| Output aggregation | No | No | No | AGENT-029 |
| Cancellation + cleanup | Partial | Yes (abort) | No | AGENT-030 |

## Appendix B: ralph.json story mapping

| Story | Requirement | Summary |
|-------|------------|---------|
| AGENT-016 | REQ-036 | Context fork spawn |
| AGENT-017 | REQ-036 | Fresh context spawn |
| AGENT-018 | REQ-036 | Multi-provider routing |
| AGENT-019 | REQ-036 | Resume failed subagent |
| AGENT-020 | REQ-036 | Retry with exponential backoff |
| AGENT-021 | REQ-036 | Model switch mid-session |
| AGENT-022 | REQ-036 | Context compression |
| AGENT-023 | REQ-036 | Structured handoff |
| AGENT-024 | REQ-036 | Auto compression toggle |
| AGENT-025 | REQ-036 | Dedicated settings section |
| AGENT-026 | REQ-036 | Pool manager |
| AGENT-027 | REQ-036 | Observability dashboard |
| AGENT-028 | REQ-036 | Permission inheritance |
| AGENT-029 | REQ-036 | Output aggregation |
| AGENT-030 | REQ-036 | Cancellation and cleanup |
