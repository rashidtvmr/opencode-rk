# Scratchpad: claude-code harness reference (lean-feature mining)

## Claim ledger

### C1. Single-binary CLI, pipeline architecture
- Source evidence: `/home/rashid/projects/tmpcodes/claude-code/docs/architecture.md` (pipeline: User Input > CLI Parser > Query Engine > LLM API > Tool Execution Loop > Terminal UI); `src/main.tsx` 785K as bundle entry; `src/entrypoints/cli.tsx` 38.4K CLI orchestration. Commit `eec3692193a30bdedb9f31e033d93971aec73585`. opencode-rk commit `a754746dbaeb136286ef1862017ae49e68e89289`.
- Observed scenario: one REPL screen (`src/screens/REPL.tsx` 874K), Commander.js arg parsing (`@commander-js/extra-typings` in package.json), Ink React terminal UI.
- Target boundary: opencode-rk PLAN ADR-001/ADR-002 (one Tokio daemon, many clients), stories BASE-003/004/005, UI-009.
- Tests: `rtk git rev-parse HEAD` both repos PASS; `rtk ls src` PASS; read package.json version `0.0.0-leaked`.
- Decision: keep daemon+TUI split; do not copy Ink/React. Lean value is pipeline separation (parse > query engine > tool loop), not the UI toolkit.
- Unknown: exact REPL state-machine transitions.

### C2. QueryEngine = agent loop with tool-call loop, retry, token tracking
- Source evidence: `src/QueryEngine.ts:1-60` (imports query, cost-tracker, canUseTool, memory prompt, fileHistory snapshot); `src/query.ts:1-80` (message normalization, compact hooks, attachment prefetch, queue manager); `docs/architecture.md` section 3.
- Observed scenario: streaming LLM response, tool execution, result fed back; `categorizeRetryableAPIError` import in QueryEngine; `tokenCountWithEstimation` in autoCompact.
- Target boundary: AGENT family (AGENT-002/003/004/005), ROUTE fallback.
- Tests: read-only inspection, no execution (leaked TS, no build attempted).
- Decision: opencode-rk agent loop should mirror: normalize > stream > tool loop > usage accumulate > compact check. Retry classification is NEW-REQ candidate.
- Unknown: exact retry backoff values (grep withRetry.ts returned empty, file layout differs).

### C3. Tool contract via buildTool factory
- Source evidence: `src/Tool.ts:704,717,783` (`buildTool` fills defaults for ToolDef); tool dirs: `AgentTool AskUserQuestionTool BashTool FileEditTool FileReadTool FileWriteTool GlobTool GrepTool LSPTool TodoWriteTool TaskCreate/Get/List/Output/Stop/UpdateTool TeamCreate/DeleteTool SkillTool ToolSearchTool EnterPlanMode/ExitPlanModeTool WebFetch/SearchTool NotebookEditTool ScheduleCronTool SleepTool SendMessageTool` (~40 tools).
- Observed scenario: each tool dir has Tool + prompt + constants + UI; `ToolSearchTool` enables just-in-time tool discovery.
- Target boundary: TOOL family, EXT-001/002.
- Tests: `rtk ls src/tools` PASS.
- Decision: adopt factory pattern (schema + permission + exec + render in one def) and a ToolSearch equivalent for lean context. Do not copy 40 tools 1:1.
- Unknown: `isConcurrencySafe` flag cited in docs/architecture.md but not line-verified.

### C4. Permission modes + rule sources + decision reasons
- Source evidence: `src/types/permissions.ts` (EXTERNAL_PERMISSION_MODES acceptEdits/bypassPermissions/default/dontAsk/plan; Internal adds auto/bubble; PermissionRuleSource userSettings/projectSettings/localSettings/flagSettings/policySettings/cliArg/command/session; PermissionUpdateDestination; PermissionDecisionReason rule/mode/subcommandResults/permissionPromptTool/hook/asyncAgent/sandboxOverride/classifier/workingDir/safetyCheck/other); `src/utils/permissions/PermissionMode.ts:45-91` mode config table.
- Observed scenario: plan mode pauses edits; bypass/dontAsk marked error color; auto is ant-only.
- Target boundary: SEC-001/003/017, REQ-021/029/030, UI-008/011.
- Tests: read both files fully.
- Decision: adopt layered sources + typed decision reasons verbatim concept. opencode-rk already mandates star-cannot-bypass; this gives the enum to enforce it.
- Unknown: none material.

### C5. Dangerous-pattern denylist for shell allow-rules
- Source evidence: `src/utils/permissions/dangerousPatterns.ts:18-42` CROSS_PLATFORM_CODE_EXEC (python/node/deno/tsx/ruby/perl/php/lua/npx/bunx/npm-run/yarn/pnpm/bun-run/bash/sh/ssh); :44-80 DANGEROUS_BASH_PATTERNS plus ant-only (gh/curl/wget/git/kubectl/aws/gcloud); predicates in permissionSetup.ts strip such rules at auto-mode entry.
- Observed scenario: `Bash(python:*)` treated as arbitrary-code bypass, stripped on auto-mode entry.
- Target boundary: SEC-002/012/016 (REQ-025), SEC-013 (REQ-026).
- Tests: read file fully.
- Decision: port the cross-platform list as deterministic denylist; keep ant-only cloud list out (no usage data). Small, high-value.
- Unknown: exact matcher shape variants verified only via header comment.

### C6. Hook event system, bounded, allowlisted
- Source evidence: `src/entrypoints/sdk/coreTypes.ts:25` HOOK_EVENTS (PreToolUse/PostToolUse/PostToolUseFailure/Notification/UserPromptSubmit/SessionStart/SessionEnd/Stop/StopFailure/SubagentStart/SubagentStop/PreCompact/PostCompact/PermissionRequest/PermissionDenied/Setup/TeammateIdle/TaskCreated/TaskCompleted/...); `src/utils/hooks/hookEvents.ts:18,20` (ALWAYS_EMITTED SessionStart/Setup; MAX_PENDING_EVENTS=100 with shift-drop); exec helpers `execAgentHook/execHttpHook/execPromptHook`, `ssrfGuard.ts` 8.5K, `sessionHooks.ts` FunctionHook session-scoped non-persisted.
- Observed scenario: hooks get allow/deny before auto-deny fallback (permissions.ts:394 comment); progress via interval 1000ms with dedupe.
- Target boundary: SEC-010/011 (REQ-020), EXT-008.
- Tests: read hookEvents.ts fully; grep permissions.ts:394-444.
- Decision: adopt bounded pending queue (100) + always-emit allowlist + SSRF guard as mandatory shape. Matches AGENTS.md no-unbounded-queue rule directly.
- Unknown: hook timeout defaults.

### C7. Auto-compact with thresholds, warning states, circuit breaker
- Source evidence: `src/services/compact/autoCompact.ts` (MAX_OUTPUT_TOKENS_FOR_SUMMARY 20000; AUTOCOMPACT_BUFFER 13000; WARNING/ERROR buffers 20000; MANUAL buffer 3000; MAX_CONSECUTIVE_FAILURES=3 with BQ comment 1279 sessions/250K wasted calls; getEffectiveContextWindowSize; calculateTokenWarningState; session-memory-first then legacy compactConversation; DISABLE_COMPACT/DISABLE_AUTO_COMPACT env).
- Observed scenario: proactive compact at effective-13k; circuit breaker stops doomed retries; manual /compact preserved.
- Target boundary: SESS family (SESS-001/017), OPS-001/007 memory bounds.
- Tests: read file fully.
- Decision: adopt thresholds + breaker + env kill-switches. Session-memory-first is optional later phase.
- Unknown: tokenCountWithEstimation accuracy.

### C8. Cost tracker per model + per turn
- Source evidence: `src/cost-tracker.ts:1-60` (re-exports bootstrap state counters: total cost USD, durations incl. without-retries, input/output/cache-read/cache-create tokens, lines added/removed, web-search requests, unknown-model-cost flag); `/cost` LocalCommand.
- Observed scenario: `/cost` prints turn/conversation totals.
- Target boundary: UI-006/ROUTE-008 (REQ-016), ROUTE cost-aware routing.
- Tests: read header only.
- Decision: adopt counter set as telemetry schema; unknown-model-cost flag prevents silent misbilling.
- Unknown: pricing table source (modelCost.ts unread).

### C9. Skills: bundled registry + safe extraction + frontmatter hooks
- Source evidence: `src/skills/bundledSkills.ts` (registerBundledSkill; lazy extract to disk once per process via memoized promise; O_NOFOLLOW|O_EXCL 0o600 writes; resolveSkillFilePath rejects `..`/absolute; Base-directory prefix so model Reads on demand); `loadSkillsDir.ts` 33.6K; `mcpSkillBuilders.ts`; hooks `registerFrontmatterHooks/registerSkillHooks`.
- Observed scenario: compiled-in skills materialize reference files owner-only on first use.
- Target boundary: EXT-001/002 (REQ-017), UI-010.
- Tests: read bundledSkills.ts fully.
- Decision: adopt safe-extract pattern verbatim (mode bits + traversal reject). Directly satisfies SEC posture for skill content.
- Unknown: frontmatter schema fields (grep returned empty due to rtk chaining).

### C10. Built-in agents: General/Explore/Plan/Guide/Statusline/Verification
- Source evidence: `src/tools/AgentTool/builtInAgents.ts` (getBuiltInAgents; env kill-switch CLAUDE_AGENT_SDK_DISABLE_BUILTIN_AGENTS; Explore+Plan gated by experiment flag; Guide excluded from SDK entrypoints; coordinator worker agents behind COORDINATOR_MODE flag).
- Observed scenario: Explore (read-only search), Plan (read-only planner), General (executor), Verification (gated experiment), statusline-setup.
- Target boundary: AGENT-002/006/010 (REQ-009/018), AGENT-015 lean.
- Tests: read via rtk cat PASS.
- Decision: adopt 3-role split (Explore/Plan/Execute) as lean default; verification agent deferred.
- Unknown: per-agent system prompts (prompt.ts 16.3K unread).

### C11. Plan mode as permission mode + Enter/Exit tools
- Source evidence: `src/tools/EnterPlanModeTool/` + `ExitPlanModeTool/` dirs; PermissionMode `plan` title "Plan Mode" with pause icon (`PermissionMode.ts:52-58`).
- Observed scenario: model invokes EnterPlanMode to propose without edits; user approves to exit.
- Target boundary: AGENT-007/008 (REQ-013 steer subagents), UI-005.
- Tests: dir listing only.
- Decision: plan mode is cheapest steering primitive; propose as NEW-REQ if no story covers it.
- Unknown: enforcement point (tool-level vs broker-level).

### C12. Session persistence: per-project cache dirs, file-history snapshots
- Source evidence: `src/utils/cachePaths.ts` (envPaths claude-cli, sanitizePath djb2Hash, per-cwd project dir, errors/messages/mcp-logs subdirs); `QueryEngine.ts:50-54` fileHistory snapshot imports; `src/utils/filePersistence/filePersistence.ts` (BYOC-only outputs upload, `..` traversal skip, FILE_COUNT_LIMIT, concurrency limit).
- Observed scenario: cache names stable across upgrades; outputs scanned per turn and uploaded with cap.
- Target boundary: SESS-001/DB-003 (REQ-006), DB-001/006/008 (REQ-033), OPS disk-growth objective.
- Tests: read both files.
- Decision: adopt sanitized per-project dirs + snapshot-before-edit + count/concurrency caps. Transcript JSONL path not confirmed, recorded unknown.
- Unknown: session transcript file format/path (fileHistory.ts grep empty).

### C13. Sandbox types + MCP + SDK surfaces
- Source evidence: `src/entrypoints/sdk/coreTypes.ts:11-17` re-exports SandboxFilesystemConfig/SandboxNetworkConfig/SandboxSettings; `src/entrypoints/sandboxTypes.ts` 5.6K; `src/utils/sandbox/sandbox-adapter.ts` 34.9K; `src/services/mcp/` (client 116K, config 49K, auth 86K, elicitation, channel allowlist/permissions); `src/entrypoints/sdk/` (controlSchemas 19K, coreSchemas 55K); `src/entrypoints/mcp.ts` 6.1K (Claude Code as MCP server); Doctor screen 71K (`/doctor` env diagnostics).
- Observed scenario: sandbox config is typed API surface; MCP has channel-level permissions; /doctor checks API/auth/tools/MCP.
- Target boundary: SEC-004/005/014 (REQ-027 sandbox), EXT-005/009/012 (REQ-005 plugins), REL diagnostics.
- Tests: listings + coreTypes header only.
- Decision: adopt typed sandbox config + /doctor equivalent + MCP channel permissions. Full MCP client depth deferred.
- Unknown: sandbox backend (Landlock? seatbelt?) unverified.

### C14. Commands: ~90 slash dirs, 3-type taxonomy
- Source evidence: `src/commands/` ~90 dirs (session/resume/compact/model/permissions/cost/memory/doctor/mcp/plugin/skills/review/security-review/sandbox-toggle/...); `docs/architecture.md` taxonomy PromptCommand/LocalCommand/LocalJSXCommand; `src/commands/session/session.tsx` 12.7K, `resume.tsx` 36.2K, `model.tsx` 36.7K.
- Observed scenario: /compact /resume /rewind /teleport /export /share /stats /usage /output-style /keybindings /vim /voice.
- Target boundary: UI-004/005/010, SHARE-001/004/005, SESS-011 fork.
- Tests: `rtk ls src/commands` PASS.
- Decision: do not port 90 commands. Port taxonomy + 8 lean commands (/compact /resume /rewind /model /permissions /cost /doctor /export).
- Unknown: per-command arg schemas.

## Verification performed
- `rtk git rev-parse HEAD` in opencode-rk: `a754746...` PASS.
- `rtk git rev-parse HEAD` in harness: `eec369219...` PASS.
- `rtk ls src` in harness: 30+ dirs PASS.
- package.json: name `@anthropic-ai/claude-code`, version `0.0.0-leaked`, Bun >=1.1.0, React 19 + Ink, zod, MCP SDK, OTel, GrowthBook.
- Re-read of this file after write: pending (single write, will re-read).

## Decisions (global)
- Pipeline shape, permission enums, hook bounds, compact breaker, skill safe-extract, dangerous-pattern list: adopt.
- Ink/React, OTel/GrowthBook weight, 90 commands, 40 tools, cloud MCP depth: do not adopt.
- All proposals stay inside existing REQ families where possible; truly new items marked NEW-REQ.

## Remaining unknowns
- Session transcript JSONL path/format; retry backoff constants; hook timeout defaults; sandbox OS backend; per-agent prompts; model pricing table; command arg schemas.

## Ranked Top 8 lean suggestions

### 1. Typed permission modes + layered rule sources + decision reasons
- What: port EXTERNAL_PERMISSION_MODES, PermissionRuleSource (7 layers), PermissionUpdate destinations, PermissionDecisionReason enum.
- Lean value: 3 small enums give deterministic broker, audit log, star-cannot-bypass enforcement. Zero runtime cost.
- Cost/risk: low. Pure types + broker switch. Risk: over-fitting ant-only auto/bubble modes; exclude them.
- Maps to: REQ-021 (SEC-001/003/017), REQ-029/030.

### 2. Dangerous shell-pattern denylist stripped at auto-entry
- What: CROSS_PLATFORM_CODE_EXEC + DANGEROUS_BASH_PATTERNS as deny-strip list when entering auto/accept modes.
- Lean value: closes `Bash(python:*)`-style wildcard bypass with ~40 strings. No model call.
- Cost/risk: low. Static list, unit-testable. Risk: false positives on legit `npm run`; mitigate with exact-shape matcher.
- Maps to: REQ-025 (SEC-002/012/016), REQ-026 (SEC-013).

### 3. Bounded hook bus (100 pending, allowlisted always-emit, SSRF guard)
- What: MAX_PENDING_EVENTS=100 shift-drop, SessionStart/Setup always emitted, execAgentHook/execHttpHook/execPromptHook split, ssrfGuard on HTTP hooks, session-scoped FunctionHooks.
- Lean value: satisfies no-unbounded-queue rule with ~100 lines; hooks cannot OOM daemon.
- Cost/risk: low-medium. Needs spawn sandbox for hook commands. Risk: hook-invoked hooks recursion; cap depth 1.
- Maps to: REQ-020 (SEC-010/011, EXT-008).

### 4. Auto-compact thresholds + warning states + 3-strike circuit breaker
- What: effective-window minus 13k trigger, 20k warn/err buffers, 3k manual buffer, consecutive-failure breaker, DISABLE_COMPACT/DISABLE_AUTO_COMPACT env, /compact manual path.
- Lean value: prevents context-blowup API burn (harness BQ: 250K calls/day wasted without breaker). Directly bounds memory/cost.
- Cost/risk: medium. Needs token estimator + summarizer call. Risk: summary quality; mitigate with session-memory-first later.
- Maps to: REQ-006 (SESS-001/017), REQ-001/032 (OPS-001/007).

### 5. Safe skill extraction (O_EXCL + 0600 + traversal reject + lazy once)
- What: bundled skill files materialized owner-only on first use, memoized promise, `..`/absolute rejected, base-dir prefix in prompt.
- Lean value: skills readable on demand without bloating context; secure defaults. Small code.
- Cost/risk: low. Filesystem only. Risk: symlink races; O_NOFOLLOW+nonce dir handles it.
- Maps to: REQ-017 (EXT-001/002), REQ-027 (SEC-004/005).

### 6. Three-role built-in agents (Explore read-only / Plan read-only / Execute)
- What: Explore + Plan + General split with read-only enforcement for first two, SDK kill-switch env, Guide excluded from SDK.
- Lean value: cheapest delegation safety: read-only roles cannot mutate. Matches lean VPS story.
- Cost/risk: low-medium. Needs tool-level read-only gate per role. Risk: prompt drift; pin prompts.
- Maps to: REQ-009 (AGENT-002/010), REQ-018 (AGENT-006), REQ-024 (AGENT-015).

### 7. Plan mode as first-class permission mode with Enter/Exit tools
- What: `plan` mode + EnterPlanMode/ExitPlanMode tools; edits blocked while in plan.
- Lean value: steering without new UI; model proposes, human approves. Single broker check.
- Cost/risk: low. One mode + two tools. Risk: model stuck in plan; ExitPlanMode always allowed.
- Maps to: NEW-REQ-PLAN (extends REQ-013 AGENT-007/008, UI-005).

### 8. Per-turn cost counters + /cost + /doctor diagnostics
- What: input/output/cache-read/cache-create tokens, API duration with/without retries, lines changed, web-search count, unknown-model-cost flag; /doctor checks auth/connectivity/tools/MCP.
- Lean value: makes cost visible (routing decisions) and failures diagnosable without logs spelunking. Counters are cheap.
- Cost/risk: low. Counter struct + two commands. Risk: pricing staleness; unknown-cost flag covers it.
- Maps to: REQ-016 (UI-006/ROUTE-008), NEW-REQ-DOC (extends REL diagnostics).
