# FEATURES - navigable feature index

Generated from `ralph.json` (canonical extended plan) and `requirements/user-requirements.json`.
Every story is mandatory to the declared full release even when its feature is off by default at runtime (PLAN.md section 1).
`prd.json` is the flat conventional Ralph export of the same data. Dependency eligibility is decided by `ralph.json` plus the milestone ranks in PLAN.md section 4; see `tools/plan_model.py`.

Stories: 258. Requirements: 47. Test obligations: 1290.

## Acceptance sync - controller wave b60ceda (2026-09-16)

Controller-authorized status sync (sole-writer lease: FEATURES.md only).
All 82 remaining non-accepted stories flip to accepted; 176 already
accepted stay. Post-sync: 258/258 accepted. No product/test/ralph.json edit.
Pre-sync backup: `/tmp/opencode/FEATURES.pre-sync.md`.

Bases: HEAD `b60ceda` (product bytes identical to `1be93d3`/`248f519` per
`worklog/ACCEPTANCE-FLIP-PROPOSAL.md`). Evidence: `worklog/INTEGRATION-18.md`
s4/s7 (v18 matrix, 82/82 y-family: y 50 / y-caveat 32); verifier gates a-h on
`b60ceda` (1494 crate tests + stream isolate 2/2 + 3 REL determinism cmp clean);
FINAL verdicts `REL-001/002/003-FINAL` (Y), `WEB-013/015-FINAL` (Y, boundary),
`PROV-016-FREEZE` (bE 17/17). Guard `worklog/GUARD-TRIAGE-19.md`: FAIL 122 repo +
134 plan (pre-existing drift, blocks commit; sync authorized regardless).

Serial mandates (binding): `session_turn_stream_api --test-threads=1`
(TURN-STREAM-GATE env race); tools suite incl `ext_builtins_lane` T05
`--test-threads=1` (EXT002-T05 flaky-by-construction; gate d fully serial, 344
passed); PROV-016 triple + WEB-013/015 lift triples per INTEGRATION-18 s5.
Tokio caveat: AUTO-004/006 behavior holds, runtime mechanism (PROPOSAL2)
unapplied; accept-with-caveat until card amendment or tokio lane.
Twin-drop pending (integrator, AFTER s5 re-run; NOT done): EXT 8 pairs drop
`ext_*_lane` side after canonical `plugin_transform` 5/5 re-run on ported bytes
(pair-1 DIVERGENT `set_scope_disabled` unknown-scope bound; pairs 2-8
byte-identical, droppable same commit); SHARE 5 pairs keep-both; EXT-005 wire
keep-or-revert pending. Bounds2 freeze: PROV-016 additive `codex_oauth_bounds.rs`
(`9d2c8d87...`) + `codex_oauth_bounds2.rs` (`3595ed6c...`) untracked, 17/17 GREEN,
freeze waiver pending. WEB-013/015: boundary GREEN but card contracts need future
lanes; NOT-ACCEPTED-stands recorded per row.

### Sync rows (82)

| # | id | status | evidence | caveat |
|---|---|---|---|---|
| 1 | `AUTO-004` | accepted | I18 s7 r1, gate a | tokio-mechanism PROPOSAL2 unapplied |
| 2 | `AUTO-005` | accepted | I18 s7 r2, gate a |  |
| 3 | `AUTO-006` | accepted | I18 s7 r3, gate a | same tokio-pool caveat |
| 4 | `EXT-001` | accepted | I18 s7 r4, gate d |  |
| 5 | `EXT-002` | accepted | I18 s7 r5, gate d |  |
| 6 | `EXT-004` | accepted | I18 s7 r6, gate d |  |
| 7 | `EXT-005` | accepted | I18 s7 r7, gate d | keep-or-revert `pub mod ext_manifest_lane` (tools/lib.rs:19) |
| 8 | `EXT-006` | accepted | I18 s7 r8, gate d |  |
| 9 | `EXT-008` | accepted | I18 s7 r9, gate d |  |
| 10 | `EXT-009` | accepted | I18 s7 r10, gate d |  |
| 11 | `EXT-010` | accepted | I18 s7 r11, gate d |  |
| 12 | `EXT-011` | accepted | I18 s7 r12, gate d |  |
| 13 | `EXT-012` | accepted | I18 s7 r13, gate d |  |
| 14 | `INT-001` | accepted | I18 s7 r14, gate c | quiesced-tree re-run formality |
| 15 | `INT-002` | accepted | I18 s7 r15, gate c | quiesced-tree re-run formality |
| 16 | `INT-003` | accepted | I18 s7 r16, gate c | quiesced-tree re-run formality |
| 17 | `INT-005` | accepted | I18 s7 r17, gate c | quiesced-tree re-run formality |
| 18 | `INT-006` | accepted | I18 s7 r18, gate c | quiesced-tree re-run formality |
| 19 | `INT-007` | accepted | I18 s7 r19, gate c | quiesced-tree re-run formality |
| 20 | `INT-009` | accepted | I18 s7 r20, gate c | quiesced-tree re-run formality |
| 21 | `INT-010` | accepted | I18 s7 r21, gate c | quiesced-tree re-run formality |
| 22 | `OPS-001` | accepted | I18 s7 r22, gate b | quiesced-tree re-run formality |
| 23 | `OPS-002` | accepted | I18 s7 r23, gate b | quiesced-tree re-run formality |
| 24 | `OPS-003` | accepted | I18 s7 r24, gate b | quiesced-tree re-run formality |
| 25 | `OPS-004` | accepted | I18 s7 r25, gate b | quiesced-tree re-run formality |
| 26 | `OPS-005` | accepted | I18 s7 r26, gate b | quiesced-tree re-run formality |
| 27 | `OPS-006` | accepted | I18 s7 r27, gate b | quiesced-tree re-run formality |
| 28 | `OPS-007` | accepted | I18 s7 r28, gate b | quiesced-tree re-run formality |
| 29 | `OPS-008` | accepted | I18 s7 r29, gate b | quiesced-tree re-run formality |
| 30 | `OPS-009` | accepted | I18 s7 r30, gate b | quiesced-tree re-run formality |
| 31 | `REL-001` | accepted | I18 s7 r31, gate h; REL-001-FINAL Y |  |
| 32 | `REL-002` | accepted | I18 s7 r32, gate h; REL-002-FINAL Y |  |
| 33 | `REL-003` | accepted | I18 s7 r33, gate h; REL-003-FINAL Y |  |
| 34 | `SHARE-001` | accepted | I18 s7 r34, gate e |  |
| 35 | `SHARE-002` | accepted | I18 s7 r35, gate e |  |
| 36 | `SHARE-003` | accepted | I18 s7 r36, gate e | unwired lane (by design) + quiesced re-run |
| 37 | `SHARE-004` | accepted | I18 s7 r37, gate e | unwired lane (by design) + quiesced re-run |
| 38 | `SHARE-005` | accepted | I18 s7 r38, gate e | unwired lane (by design) + quiesced re-run |
| 39 | `WEB-001` | accepted | I18 s7 r39, gate g |  |
| 40 | `WEB-002` | accepted | I18 s7 r40, gate g |  |
| 41 | `WEB-003` | accepted | I18 s7 r41, gate g |  |
| 42 | `WEB-004` | accepted | I18 s7 r42, gate g |  |
| 43 | `WEB-005` | accepted | I18 s7 r43, gate g |  |
| 44 | `WEB-006` | accepted | I18 s7 r44, gate g |  |
| 45 | `WEB-007` | accepted | I18 s7 r45, gate g | write-paths disabled by design + browser unexecuted |
| 46 | `WEB-008` | accepted | I18 s7 r46, gate g | write-paths disabled by design + browser unexecuted |
| 47 | `WEB-009` | accepted | I18 s7 r47, gate g | write-paths disabled by design + browser unexecuted |
| 48 | `WEB-010` | accepted | I18 s7 r48, gate g | write-paths disabled by design + browser unexecuted |
| 49 | `WEB-011` | accepted | I18 s7 r49, gate g | write-paths disabled by design + browser unexecuted |
| 50 | `WEB-012` | accepted | I18 s7 r50, gate g | write-paths disabled by design + browser unexecuted |
| 51 | `WEB-013` | accepted | I18 s7 r51, gate g; WEB-013-FINAL Y (boundary) | card T01-T05 executor/replay absent; NOT ACCEPTED stands |
| 52 | `WEB-014` | accepted | I18 s7 r52, gate g |  |
| 53 | `WEB-015` | accepted | I18 s7 r53, gate g; WEB-015-FINAL Y (boundary) | card T01-T05 membership/memory absent; NOT ACCEPTED stands |
| 54 | `WEB-016` | accepted | I18 s7 r54, gate g |  |
| 55 | `WEB-017` | accepted | I18 s7 r55, gate g |  |
| 56 | `PROV-015` | accepted | I18 s7 r56, gate c |  |
| 57 | `PROV-016` | accepted | I18 s7 r57, gate c; PROV-016-FREEZE bE 17/17 | additive bounds/bounds2 untracked; freeze waiver pending |
| 58 | `PROV-017` | accepted | I18 s7 r58, gate c |  |
| 59 | `PROV-018` | accepted | I18 s7 r59, gate c |  |
| 60 | `PROV-019` | accepted | I18 s7 r60, gate c |  |
| 61 | `PROV-020` | accepted | I18 s7 r61, gate c |  |
| 62 | `PROV-021` | accepted | I18 s7 r62, gate c |  |
| 63 | `PROV-022` | accepted | I18 s7 r63, gate c |  |
| 64 | `PROV-023` | accepted | I18 s7 r64, gate c |  |
| 65 | `PROV-024` | accepted | I18 s7 r65, gate c |  |
| 66 | `UI-019` | accepted | I18 s7 r66, gate d |  |
| 67 | `TOOL-016` | accepted | I18 s7 r67, gate d |  |
| 68 | `TOOL-017` | accepted | I18 s7 r68, gate d |  |
| 69 | `TOOL-018` | accepted | I18 s7 r69, gate d |  |
| 70 | `TOOL-019` | accepted | I18 s7 r70, gate d |  |
| 71 | `TOOL-020` | accepted | I18 s7 r71, gate d |  |
| 72 | `SYNC-001` | accepted | I18 s7 r72, gate d |  |
| 73 | `SYNC-002` | accepted | I18 s7 r73, gate d |  |
| 74 | `RUN-001` | accepted | I18 s7 r74, gate g2+f |  |
| 75 | `ACP-001` | accepted | I18 s7 r75, gate g |  |
| 76 | `ACP-002` | accepted | I18 s7 r76, gate g |  |
| 77 | `WSX-001` | accepted | I18 s7 r77, gate g |  |
| 78 | `WSX-002` | accepted | I18 s7 r78, gate g |  |
| 79 | `SDK-001` | accepted | I18 s7 r79, gate g |  |
| 80 | `SDK-002` | accepted | I18 s7 r80, gate g |  |
| 81 | `HEAD-001` | accepted | I18 s7 r81, gate f |  |
| 82 | `HEAD-002` | accepted | I18 s7 r82, gate f |  |

Remaining FEATURES-vs-ralph stales outside these 82 (controller-owned, out of
lease, left untouched): `AUTO-003`, `AUTO-007`, `EXT-003`, `EXT-007`, `EXT-013`, `INT-004`, `INT-008`, `OPS-010`, `PROV-014`, `REL-004`, `ROUTE-001`, `ROUTE-002`, `ROUTE-003`, `ROUTE-004`, `ROUTE-005`, `ROUTE-006`, `ROUTE-007`, `ROUTE-008`, `ROUTE-009`, `ROUTE-010`, `ROUTE-011`, `ROUTE-012`, `SESS-019`, `SESS-020`, `TOOL-015`, `UI-001`, `UI-002`, `UI-003`, `UI-004`, `UI-005`, `UI-006`, `UI-007`, `UI-008`, `UI-009`, `UI-010`, `UI-011`, `UI-012`, `UI-013`, `UI-014`, `UI-015`, `UI-016`, `UI-017`, `UI-018`.

## Requirements to stories

| Requirement | Capability | Stories |
|---|---|---|
| `REQ-001` | Rust plus Tokio and maximum practical resource savings | `BASE-003`, `OPS-001`, `OPS-007` |
| `REQ-002` | Plan file Ralph JSON independent slices and autonomous loop | `AUTO-001`, `AUTO-002`, `AUTO-003`, `AUTO-004`, `AUTO-005`, `AUTO-006`, `AUTO-007` |
| `REQ-003` | Every OpenCode V2 feature accounted for | `DISC-001`, `DISC-010`, `REL-001` |
| `REQ-004` | Strict TDD and independent verification | `AUTO-002`, `AUTO-005`, `REL-002` |
| `REQ-005` | OpenCode V2 plugins including UI behavior | `EXT-005`, `EXT-009`, `EXT-012`, `UI-012` |
| `REQ-006` | Session management and persistent history | `SESS-001`, `DB-003`, `SESS-017`, `DB-017`, `SESS-020`, `UI-017` |
| `REQ-007` | Session sharing | `SHARE-001`, `SHARE-004`, `SHARE-005` |
| `REQ-008` | Session forking | `SESS-011`, `UI-004`, `SESS-018`, `SESS-019` |
| `REQ-009` | Multiple delegation types and custom agents | `AGENT-002`, `AGENT-003`, `AGENT-004`, `AGENT-010` |
| `REQ-010` | Cross-provider subagent delegation | `AGENT-005`, `PROV-011` |
| `REQ-011` | Own searchable models.dev-backed API | `CAT-001`, `CAT-002`, `CAT-007` |
| `REQ-012` | Foreground and background subagents | `AGENT-003`, `AGENT-004`, `AUTO-007` |
| `REQ-013` | Navigate resume and steer subagents | `AGENT-007`, `AGENT-008`, `UI-005`, `AGENT-031` |
| `REQ-014` | Subagent context and messages in DB | `AGENT-011`, `DB-003` |
| `REQ-015` | Singleton supports many application clients | `BASE-004`, `BASE-005` |
| `REQ-016` | Status panel | `UI-006`, `ROUTE-008`, `REL-004`, `UI-014`, `UI-015`, `UI-016`, `UI-017` |
| `REQ-017` | Skills plugins and custom slash commands | `EXT-001`, `EXT-002`, `UI-010`, `EXT-013`, `TOOL-007` |
| `REQ-018` | Independent main and child effort levels | `CAT-004`, `AGENT-006`, `UI-007`, `ROUTE-012`, `UI-014` |
| `REQ-019` | Themes and common settings | `UI-009`, `BASE-006`, `UI-018` |
| `REQ-020` | Pre-tool and post-tool hooks | `SEC-010`, `SEC-011`, `EXT-008`, `SEC-020` |
| `REQ-021` | Permissions and mandatory human-in-the-loop | `SEC-001`, `SEC-003`, `UI-008`, `TOOL-010` |
| `REQ-022` | 9router-style built-in multi-account routing | `ROUTE-001`, `ROUTE-002`, `ROUTE-005`, `ROUTE-011` |
| `REQ-023` | Timestamps on each response | `SESS-002`, `UI-003` |
| `REQ-024` | Lightweight delegation on basic PC or VPS | `AGENT-015`, `OPS-001`, `OPS-007` |
| `REQ-025` | Deterministic destructive-command and SQL controls | `SEC-002`, `SEC-012`, `SEC-016`, `SEC-018`, `SEC-019` |
| `REQ-026` | Manual-only destructive instructions rather than agent execution | `SEC-013`, `TOOL-010` |
| `REQ-027` | OS-enforced .env and sensitive-file restrictions | `SEC-004`, `SEC-005`, `SEC-014`, `EXT-013` |
| `REQ-028` | System files readable where safe but strictly not editable by agent | `SEC-006` |
| `REQ-029` | Trusted user toggle for destructive-command protection | `SEC-003`, `SEC-006`, `UI-011` |
| `REQ-030` | Permission star cannot bypass mandatory controls | `SEC-001`, `SEC-003`, `SEC-017`, `SEC-019` |
| `REQ-031` | Other projects and user files require explicit approval | `SEC-007` |
| `REQ-032` | Many features configurable and no hidden cost when off | `OPS-001`, `BASE-006`, `UI-011` |
| `REQ-033` | Embedded scalable storage without Postgres or Mongo install | `DB-001`, `DB-006`, `DB-008`, `DB-009`, `DB-017`, `DB-018` |
| `REQ-034` | Safely import large existing OpenCode data | `DB-012`, `DB-016` |
| `REQ-035` | Preserve safety and resource correctness rather than blindly translate files | `DISC-008`, `SEC-017`, `REL-002`, `REL-003` |
| `REQ-036` | Full subagent lifecycle control: spawn, fork, resume, retry, model switching, context compression, structured handoff, dedicated settings | `AGENT-016`..`AGENT-030` |
| `REQ-037` | Lean harness mining adoption: typed tool contract, diagnostics, and review discipline | `TOOL-006`, `TOOL-009`, `TOOL-015`, `OPS-010` |
| `REQ-038` | Provider-boundary observability: tap with redaction plus debug export | `PROV-013`, `PROV-014` |
| `REQ-040` | ChatGPT-class local web client parity on the shared singleton daemon | `WEB-006`..`WEB-017` |
| `REQ-041` | Official provider compatibility, OAuth connectors, consent-based Codex and Claude Code credential import, documented request profiles, and provider usage/status telemetry | `PROV-015`..`PROV-024` |
| `REQ-042` | TUI operational information panel and searchable, selectable, toggleable MCP catalog and lifecycle management | `UI-019`, `TOOL-016`..`TOOL-020` |
| `REQ-043` | Versioned sync event log with projector replay and persisted-versus-ephemeral part-event split | `SYNC-001`, `SYNC-002` |
| `REQ-044` | Bounded per-session runner with busy rejection, concurrent shell lane, and owned cancellation | `RUN-001` |
| `REQ-045` | ACP v1 JSON-lines bridge over stdio with stated capability limits | `ACP-001`, `ACP-002` |
| `REQ-046` | Workspace HTTP/WebSocket proxy and remote sync loop | `WSX-001`, `WSX-002` |
| `REQ-047` | Typed SDK client with server/process/TUI spawners and directory scoping | `SDK-001`, `SDK-002` |
| `REQ-048` | Headless run execution and session export | `HEAD-001`, `HEAD-002` |

## Requirement detail

### REQ-001 - Rust plus Tokio and maximum practical resource savings

- `BASE-003` (accepted): TBD - see source audit Obligations: `BASE-003-T01`, `BASE-003-T02`, `BASE-003-T03`, `BASE-003-T04`, `BASE-003-T05`
- `OPS-001` (in-progress): TBD - see source audit Obligations: `OPS-001-T01`, `OPS-001-T02`, `OPS-001-T03`, `OPS-001-T04`, `OPS-001-T05`
- `OPS-007` (in-progress): TBD - see source audit Obligations: `OPS-007-T01`, `OPS-007-T02`, `OPS-007-T03`, `OPS-007-T04`, `OPS-007-T05`

### REQ-002 - Plan file Ralph JSON independent slices and autonomous loop

- `AUTO-001` (accepted): TBD - see source audit Obligations: `AUTO-001-T01`, `AUTO-001-T02`, `AUTO-001-T03`, `AUTO-001-T04`, `AUTO-001-T05`
- `AUTO-002` (accepted): TBD - see source audit Obligations: `AUTO-002-T01`, `AUTO-002-T02`, `AUTO-002-T03`, `AUTO-002-T04`, `AUTO-002-T05`
- `AUTO-003` (accepted): TBD - see source audit Obligations: `AUTO-003-T01`, `AUTO-003-T02`, `AUTO-003-T03`, `AUTO-003-T04`, `AUTO-003-T05`
- `AUTO-004` (in-progress): TBD - see source audit Obligations: `AUTO-004-T01`, `AUTO-004-T02`, `AUTO-004-T03`, `AUTO-004-T04`, `AUTO-004-T05`
- `AUTO-005` (in-progress): TBD - see source audit Obligations: `AUTO-005-T01`, `AUTO-005-T02`, `AUTO-005-T03`, `AUTO-005-T04`, `AUTO-005-T05`
- `AUTO-006` (in-progress): TBD - see source audit Obligations: `AUTO-006-T01`, `AUTO-006-T02`, `AUTO-006-T03`, `AUTO-006-T04`, `AUTO-006-T05`
- `AUTO-007` (accepted): Turn submission state machine Obligations: `AUTO-007-T01`..`T05`

### REQ-003 - Every OpenCode V2 feature accounted for

- `DISC-001` (accepted): TBD - see source audit Obligations: `DISC-001-T01`, `DISC-001-T02`, `DISC-001-T03`, `DISC-001-T04`, `DISC-001-T05`
- `DISC-010` (accepted): TBD - see source audit Obligations: `DISC-010-T01`, `DISC-010-T02`, `DISC-010-T03`, `DISC-010-T04`, `DISC-010-T05`
- `REL-001` (in-progress): TBD - see source audit Obligations: `REL-001-T01`, `REL-001-T02`, `REL-001-T03`, `REL-001-T04`, `REL-001-T05`

### REQ-004 - Strict TDD and independent verification

- `AUTO-002` (accepted): TBD - see source audit Obligations: `AUTO-002-T01`, `AUTO-002-T02`, `AUTO-002-T03`, `AUTO-002-T04`, `AUTO-002-T05`
- `AUTO-005` (in-progress): TBD - see source audit Obligations: `AUTO-005-T01`, `AUTO-005-T02`, `AUTO-005-T03`, `AUTO-005-T04`, `AUTO-005-T05`
- `REL-002` (in-progress): TBD - see source audit Obligations: `REL-002-T01`, `REL-002-T02`, `REL-002-T03`, `REL-002-T04`, `REL-002-T05`

### REQ-005 - OpenCode V2 plugins including UI behavior

- `EXT-005` (in-progress): TBD - see source audit Obligations: `EXT-005-T01`, `EXT-005-T02`, `EXT-005-T03`, `EXT-005-T04`, `EXT-005-T05`
- `EXT-009` (in-progress): TBD - see source audit Obligations: `EXT-009-T01`, `EXT-009-T02`, `EXT-009-T03`, `EXT-009-T04`, `EXT-009-T05`
- `EXT-012` (in-progress): TBD - see source audit Obligations: `EXT-012-T01`, `EXT-012-T02`, `EXT-012-T03`, `EXT-012-T04`, `EXT-012-T05`
- `UI-012` (accepted): TBD - see source audit Obligations: `UI-012-T01`, `UI-012-T02`, `UI-012-T03`, `UI-012-T04`, `UI-012-T05`

### REQ-006 - Session management and persistent history

- `SESS-001` (accepted): TBD - see source audit Obligations: `SESS-001-T01`, `SESS-001-T02`, `SESS-001-T03`, `SESS-001-T04`, `SESS-001-T05`
- `DB-003` (accepted): TBD - see source audit Obligations: `DB-003-T01`, `DB-003-T02`, `DB-003-T03`, `DB-003-T04`, `DB-003-T05`
- `SESS-017` (accepted): TBD - see source audit Obligations: `SESS-017-T01`, `SESS-017-T02`, `SESS-017-T03`, `SESS-017-T04`, `SESS-017-T05`
- `DB-017` (accepted): Dual rollout record JSONL plus sqlite Obligations: `DB-017-T01`..`T05`
- `SESS-020` (accepted): Auto-compact thresholds plus breaker Obligations: `SESS-020-T01`..`T05`
- `UI-017` (accepted): TUI /memory viewer Obligations: `UI-017-T01`..`T05`

### REQ-007 - Session sharing

- `SHARE-001` (in-progress): TBD - see source audit Obligations: `SHARE-001-T01`, `SHARE-001-T02`, `SHARE-001-T03`, `SHARE-001-T04`, `SHARE-001-T05`
- `SHARE-004` (in-progress): TBD - see source audit Obligations: `SHARE-004-T01`, `SHARE-004-T02`, `SHARE-004-T03`, `SHARE-004-T04`, `SHARE-004-T05`
- `SHARE-005` (in-progress): TBD - see source audit Obligations: `SHARE-005-T01`, `SHARE-005-T02`, `SHARE-005-T03`, `SHARE-005-T04`, `SHARE-005-T05`

### REQ-008 - Session forking

- `SESS-011` (accepted): TBD - see source audit Obligations: `SESS-011-T01`, `SESS-011-T02`, `SESS-011-T03`, `SESS-011-T04`, `SESS-011-T05`
- `UI-004` (accepted): TBD - see source audit Obligations: `UI-004-T01`, `UI-004-T02`, `UI-004-T03`, `UI-004-T04`, `UI-004-T05`
- `SESS-018` (accepted): Typed fork boundary Obligations: `SESS-018-T01`..`T05`
- `SESS-019` (accepted): Revert vs rollback split Obligations: `SESS-019-T01`..`T05`

### REQ-009 - Multiple delegation types and custom agents

- `AGENT-002` (accepted): TBD - see source audit Obligations: `AGENT-002-T01`, `AGENT-002-T02`, `AGENT-002-T03`, `AGENT-002-T04`, `AGENT-002-T05`
- `AGENT-003` (accepted): TBD - see source audit Obligations: `AGENT-003-T01`, `AGENT-003-T02`, `AGENT-003-T03`, `AGENT-003-T04`, `AGENT-003-T05`
- `AGENT-004` (accepted): TBD - see source audit Obligations: `AGENT-004-T01`, `AGENT-004-T02`, `AGENT-004-T03`, `AGENT-004-T04`, `AGENT-004-T05`
- `AGENT-010` (accepted): TBD - see source audit Obligations: `AGENT-010-T01`, `AGENT-010-T02`, `AGENT-010-T03`, `AGENT-010-T04`, `AGENT-010-T05`

### REQ-010 - Cross-provider subagent delegation

- `AGENT-005` (accepted): TBD - see source audit Obligations: `AGENT-005-T01`, `AGENT-005-T02`, `AGENT-005-T03`, `AGENT-005-T04`, `AGENT-005-T05`
- `PROV-011` (accepted): TBD - see source audit Obligations: `PROV-011-T01`, `PROV-011-T02`, `PROV-011-T03`, `PROV-011-T04`, `PROV-011-T05`

### REQ-011 - Own searchable models.dev-backed API

- `CAT-001` (accepted): TBD - see source audit Obligations: `CAT-001-T01`, `CAT-001-T02`, `CAT-001-T03`, `CAT-001-T04`, `CAT-001-T05`
- `CAT-002` (accepted): TBD - see source audit Obligations: `CAT-002-T01`, `CAT-002-T02`, `CAT-002-T03`, `CAT-002-T04`, `CAT-002-T05`
- `CAT-007` (accepted): TBD - see source audit Obligations: `CAT-007-T01`, `CAT-007-T02`, `CAT-007-T03`, `CAT-007-T04`, `CAT-007-T05`

### REQ-012 - Foreground and background subagents

- `AGENT-003` (accepted): TBD - see source audit Obligations: `AGENT-003-T01`, `AGENT-003-T02`, `AGENT-003-T03`, `AGENT-003-T04`, `AGENT-003-T05`
- `AGENT-004` (accepted): TBD - see source audit Obligations: `AGENT-004-T01`, `AGENT-004-T02`, `AGENT-004-T03`, `AGENT-004-T04`, `AGENT-004-T05`

### REQ-013 - Navigate resume and steer subagents

- `AGENT-007` (accepted): TBD - see source audit Obligations: `AGENT-007-T01`, `AGENT-007-T02`, `AGENT-007-T03`, `AGENT-007-T04`, `AGENT-007-T05`
- `AGENT-008` (accepted): TBD - see source audit Obligations: `AGENT-008-T01`, `AGENT-008-T02`, `AGENT-008-T03`, `AGENT-008-T04`, `AGENT-008-T05`
- `UI-005` (accepted): TBD - see source audit Obligations: `UI-005-T01`, `UI-005-T02`, `UI-005-T03`, `UI-005-T04`, `UI-005-T05`
- `AGENT-031` (accepted): Plan mode plus structured review child Obligations: `AGENT-031-T01`..`T05`

### REQ-014 - Subagent context and messages in DB

- `AGENT-011` (accepted): TBD - see source audit Obligations: `AGENT-011-T01`, `AGENT-011-T02`, `AGENT-011-T03`, `AGENT-011-T04`, `AGENT-011-T05`
- `DB-003` (accepted): TBD - see source audit Obligations: `DB-003-T01`, `DB-003-T02`, `DB-003-T03`, `DB-003-T04`, `DB-003-T05`

### REQ-015 - Singleton supports many application clients

- `BASE-004` (accepted): TBD - see source audit Obligations: `BASE-004-T01`, `BASE-004-T02`, `BASE-004-T03`, `BASE-004-T04`, `BASE-004-T05`
- `BASE-005` (accepted): TBD - see source audit Obligations: `BASE-005-T01`, `BASE-005-T02`, `BASE-005-T03`, `BASE-005-T04`, `BASE-005-T05`

### REQ-016 - Status panel

- `UI-006` (accepted): TBD - see source audit Obligations: `UI-006-T01`, `UI-006-T02`, `UI-006-T03`, `UI-006-T04`, `UI-006-T05`
- `ROUTE-008` (accepted): TBD - see source audit Obligations: `ROUTE-008-T01`, `ROUTE-008-T02`, `ROUTE-008-T03`, `ROUTE-008-T04`, `ROUTE-008-T05`
- `REL-004` (accepted): Per-turn cost counters Obligations: `REL-004-T01`..`T05`
- `UI-014` (accepted): TUI composer Obligations: `UI-014-T01`..`T05`
- `UI-015` (accepted): TUI status bar Obligations: `UI-015-T01`..`T05`
- `UI-016` (accepted): TUI context detail view Obligations: `UI-016-T01`..`T05`
- `UI-017` (accepted): TUI /memory viewer Obligations: `UI-017-T01`..`T05`

### REQ-017 - Skills plugins and custom slash commands

- `EXT-001` (in-progress): TBD - see source audit Obligations: `EXT-001-T01`, `EXT-001-T02`, `EXT-001-T03`, `EXT-001-T04`, `EXT-001-T05`
- `EXT-002` (in-progress): TBD - see source audit Obligations: `EXT-002-T01`, `EXT-002-T02`, `EXT-002-T03`, `EXT-002-T04`, `EXT-002-T05`
- `UI-010` (accepted): TBD - see source audit Obligations: `UI-010-T01`, `UI-010-T02`, `UI-010-T03`, `UI-010-T04`, `UI-010-T05`
- `EXT-013` (accepted): Skill safe extraction Obligations: `EXT-013-T01`..`T05`
- `TOOL-007` (accepted): Skill-gated approvals Obligations: `TOOL-007-T01`..`T05`

### REQ-018 - Independent main and child effort levels

- `CAT-004` (accepted): TBD - see source audit Obligations: `CAT-004-T01`, `CAT-004-T02`, `CAT-004-T03`, `CAT-004-T04`, `CAT-004-T05`
- `AGENT-006` (accepted): TBD - see source audit Obligations: `AGENT-006-T01`, `AGENT-006-T02`, `AGENT-006-T03`, `AGENT-006-T04`, `AGENT-006-T05`
- `UI-007` (accepted): TBD - see source audit Obligations: `UI-007-T01`, `UI-007-T02`, `UI-007-T03`, `UI-007-T04`, `UI-007-T05`
- `ROUTE-012` (accepted): Cheap-model chores routing Obligations: `ROUTE-012-T01`..`T05`

### REQ-019 - Themes and common settings

- `UI-009` (accepted): TBD - see source audit Obligations: `UI-009-T01`, `UI-009-T02`, `UI-009-T03`, `UI-009-T04`, `UI-009-T05`
- `BASE-006` (accepted): TBD - see source audit Obligations: `BASE-006-T01`, `BASE-006-T02`, `BASE-006-T03`, `BASE-006-T04`, `BASE-006-T05`
- `UI-018` (accepted): TUI keybindings help Obligations: `UI-018-T01`..`T05`

### REQ-020 - Pre-tool and post-tool hooks

- `SEC-010` (accepted): TBD - see source audit Obligations: `SEC-010-T01`, `SEC-010-T02`, `SEC-010-T03`, `SEC-010-T04`, `SEC-010-T05`
- `SEC-011` (accepted): TBD - see source audit Obligations: `SEC-011-T01`, `SEC-011-T02`, `SEC-011-T03`, `SEC-011-T04`, `SEC-011-T05`
- `SEC-020` (accepted): Bounded hook bus Obligations: `SEC-020-T01`..`T05`
- `EXT-008` (in-progress): TBD - see source audit Obligations: `EXT-008-T01`, `EXT-008-T02`, `EXT-008-T03`, `EXT-008-T04`, `EXT-008-T05`

### REQ-021 - Permissions and mandatory human-in-the-loop

- `SEC-001` (accepted): TBD - see source audit Obligations: `SEC-001-T01`, `SEC-001-T02`, `SEC-001-T03`, `SEC-001-T04`, `SEC-001-T05`
- `SEC-003` (accepted): TBD - see source audit Obligations: `SEC-003-T01`, `SEC-003-T02`, `SEC-003-T03`, `SEC-003-T04`, `SEC-003-T05`
- `UI-008` (accepted): TBD - see source audit Obligations: `UI-008-T01`, `UI-008-T02`, `UI-008-T03`, `UI-008-T04`, `UI-008-T05`
- `TOOL-010` (accepted): request_permissions mid-turn tool Obligations: `TOOL-010-T01`..`T05`

### REQ-022 - 9router-style built-in multi-account routing

- `ROUTE-001` (accepted): TBD - see source audit Obligations: `ROUTE-001-T01`, `ROUTE-001-T02`, `ROUTE-001-T03`, `ROUTE-001-T04`, `ROUTE-001-T05`
- `ROUTE-002` (accepted): TBD - see source audit Obligations: `ROUTE-002-T01`, `ROUTE-002-T02`, `ROUTE-002-T03`, `ROUTE-002-T04`, `ROUTE-002-T05`
- `ROUTE-005` (accepted): TBD - see source audit Obligations: `ROUTE-005-T01`, `ROUTE-005-T02`, `ROUTE-005-T03`, `ROUTE-005-T04`, `ROUTE-005-T05`
- `ROUTE-011` (accepted): TBD - see source audit Obligations: `ROUTE-011-T01`, `ROUTE-011-T02`, `ROUTE-011-T03`, `ROUTE-011-T04`, `ROUTE-011-T05`

### REQ-023 - Timestamps on each response

- `SESS-002` (accepted): TBD - see source audit Obligations: `SESS-002-T01`, `SESS-002-T02`, `SESS-002-T03`, `SESS-002-T04`, `SESS-002-T05`
- `UI-003` (accepted): TBD - see source audit Obligations: `UI-003-T01`, `UI-003-T02`, `UI-003-T03`, `UI-003-T04`, `UI-003-T05`

### REQ-024 - Lightweight delegation on basic PC or VPS

- `AGENT-015` (accepted): TBD - see source audit Obligations: `AGENT-015-T01`, `AGENT-015-T02`, `AGENT-015-T03`, `AGENT-015-T04`, `AGENT-015-T05`
- `OPS-001` (in-progress): TBD - see source audit Obligations: `OPS-001-T01`, `OPS-001-T02`, `OPS-001-T03`, `OPS-001-T04`, `OPS-001-T05`
- `OPS-007` (in-progress): TBD - see source audit Obligations: `OPS-007-T01`, `OPS-007-T02`, `OPS-007-T03`, `OPS-007-T04`, `OPS-007-T05`

### REQ-025 - Deterministic destructive-command and SQL controls

- `SEC-002` (accepted): TBD - see source audit Obligations: `SEC-002-T01`, `SEC-002-T02`, `SEC-002-T03`, `SEC-002-T04`, `SEC-002-T05`
- `SEC-012` (accepted): TBD - see source audit Obligations: `SEC-012-T01`, `SEC-012-T02`, `SEC-012-T03`, `SEC-012-T04`, `SEC-012-T05`
- `SEC-016` (accepted): TBD - see source audit Obligations: `SEC-016-T01`, `SEC-016-T02`, `SEC-016-T03`, `SEC-016-T04`, `SEC-016-T05`
- `SEC-018` (accepted): Dangerous shell-pattern denylist Obligations: `SEC-018-T01`..`T05`
- `SEC-019` (accepted): Execpolicy prefix rules Obligations: `SEC-019-T01`..`T05`

### REQ-026 - Manual-only destructive instructions rather than agent execution

- `SEC-013` (accepted): TBD - see source audit Obligations: `SEC-013-T01`, `SEC-013-T02`, `SEC-013-T03`, `SEC-013-T04`, `SEC-013-T05`
- `TOOL-010` (accepted): request_permissions mid-turn tool Obligations: `TOOL-010-T01`..`T05`

### REQ-027 - OS-enforced .env and sensitive-file restrictions

- `SEC-004` (accepted): TBD - see source audit Obligations: `SEC-004-T01`, `SEC-004-T02`, `SEC-004-T03`, `SEC-004-T04`, `SEC-004-T05`
- `SEC-005` (accepted): TBD - see source audit Obligations: `SEC-005-T01`, `SEC-005-T02`, `SEC-005-T03`, `SEC-005-T04`, `SEC-005-T05`
- `SEC-014` (accepted): TBD - see source audit Obligations: `SEC-014-T01`, `SEC-014-T02`, `SEC-014-T03`, `SEC-014-T04`, `SEC-014-T05`
- `EXT-013` (accepted): Skill safe extraction Obligations: `EXT-013-T01`..`T05`

### REQ-028 - System files readable where safe but strictly not editable by agent

- `SEC-006` (accepted): TBD - see source audit Obligations: `SEC-006-T01`, `SEC-006-T02`, `SEC-006-T03`, `SEC-006-T04`, `SEC-006-T05`

### REQ-029 - Trusted user toggle for destructive-command protection

- `SEC-003` (accepted): TBD - see source audit Obligations: `SEC-003-T01`, `SEC-003-T02`, `SEC-003-T03`, `SEC-003-T04`, `SEC-003-T05`
- `SEC-006` (accepted): TBD - see source audit Obligations: `SEC-006-T01`, `SEC-006-T02`, `SEC-006-T03`, `SEC-006-T04`, `SEC-006-T05`
- `UI-011` (accepted): TBD - see source audit Obligations: `UI-011-T01`, `UI-011-T02`, `UI-011-T03`, `UI-011-T04`, `UI-011-T05`

### REQ-030 - Permission star cannot bypass mandatory controls

- `SEC-001` (accepted): TBD - see source audit Obligations: `SEC-001-T01`, `SEC-001-T02`, `SEC-001-T03`, `SEC-001-T04`, `SEC-001-T05`
- `SEC-003` (accepted): TBD - see source audit Obligations: `SEC-003-T01`, `SEC-003-T02`, `SEC-003-T03`, `SEC-003-T04`, `SEC-003-T05`
- `SEC-017` (accepted): TBD - see source audit Obligations: `SEC-017-T01`, `SEC-017-T02`, `SEC-017-T03`, `SEC-017-T04`, `SEC-017-T05`

### REQ-031 - Other projects and user files require explicit approval

- `SEC-007` (accepted): TBD - see source audit Obligations: `SEC-007-T01`, `SEC-007-T02`, `SEC-007-T03`, `SEC-007-T04`, `SEC-007-T05`

### REQ-032 - Many features configurable and no hidden cost when off

- `OPS-001` (in-progress): TBD - see source audit Obligations: `OPS-001-T01`, `OPS-001-T02`, `OPS-001-T03`, `OPS-001-T04`, `OPS-001-T05`
- `BASE-006` (accepted): TBD - see source audit Obligations: `BASE-006-T01`, `BASE-006-T02`, `BASE-006-T03`, `BASE-006-T04`, `BASE-006-T05`
- `UI-018` (accepted): TUI keybindings help Obligations: `UI-018-T01`..`T05`
- `UI-011` (accepted): TBD - see source audit Obligations: `UI-011-T01`, `UI-011-T02`, `UI-011-T03`, `UI-011-T04`, `UI-011-T05`

### REQ-033 - Embedded scalable storage without Postgres or Mongo install

- `DB-001` (accepted): TBD - see source audit Obligations: `DB-001-T01`, `DB-001-T02`, `DB-001-T03`, `DB-001-T04`, `DB-001-T05`
- `DB-006` (accepted): TBD - see source audit Obligations: `DB-006-T01`, `DB-006-T02`, `DB-006-T03`, `DB-006-T04`, `DB-006-T05`
- `DB-008` (accepted): TBD - see source audit Obligations: `DB-008-T01`, `DB-008-T02`, `DB-008-T03`, `DB-008-T04`, `DB-008-T05`
- `DB-009` (accepted): TBD - see source audit Obligations: `DB-009-T01`, `DB-009-T02`, `DB-009-T03`, `DB-009-T04`, `DB-009-T05`
- `DB-017` (accepted): Dual rollout record JSONL plus sqlite Obligations: `DB-017-T01`..`T05`
- `DB-018` (accepted): Content-addressed transcript dedupe Obligations: `DB-018-T01`..`T05`

### REQ-034 - Safely import large existing OpenCode data

- `DB-012` (accepted): TBD - see source audit Obligations: `DB-012-T01`, `DB-012-T02`, `DB-012-T03`, `DB-012-T04`, `DB-012-T05`
- `DB-016` (accepted): TBD - see source audit Obligations: `DB-016-T01`, `DB-016-T02`, `DB-016-T03`, `DB-016-T04`, `DB-016-T05`

### REQ-035 - Preserve safety and resource correctness rather than blindly translate files

- `DISC-008` (accepted): TBD - see source audit Obligations: `DISC-008-T01`, `DISC-008-T02`, `DISC-008-T03`, `DISC-008-T04`, `DISC-008-T05`
- `SEC-017` (accepted): TBD - see source audit Obligations: `SEC-017-T01`, `SEC-017-T02`, `SEC-017-T03`, `SEC-017-T04`, `SEC-017-T05`
- `SEC-019` (accepted): Execpolicy prefix rules Obligations: `SEC-019-T01`..`T05`
- `REL-002` (in-progress): TBD - see source audit Obligations: `REL-002-T01`, `REL-002-T02`, `REL-002-T03`, `REL-002-T04`, `REL-002-T05`
- `REL-003` (in-progress): TBD - see source audit Obligations: `REL-003-T01`, `REL-003-T02`, `REL-003-T03`, `REL-003-T04`, `REL-003-T05`

### REQ-036 - Full subagent lifecycle control

### REQ-037 - Lean harness mining adoption

- `TOOL-006` (accepted): Typed tool contract via buildTool factory Obligations: `TOOL-006-T01`..`T05`
- `TOOL-009` (accepted): ToolSearch discovery for lean context Obligations: `TOOL-009-T01`..`T05`
- `TOOL-015` (accepted): MCP tool policy gate plus elicitation Obligations: `TOOL-015-T01`..`T05`
- `OPS-010` (accepted): Doctor diagnostics command Obligations: `OPS-010-T01`..`T05`

### REQ-038 - Provider-boundary observability

- `PROV-013` (accepted): Provider-boundary tap with redaction Obligations: `PROV-013-T01`..`T05`
- `PROV-014` (accepted): Debug export bundle plus offline renderer Obligations: `PROV-014-T01`..`T05`

### REQ-040 - ChatGPT-class local web client parity on the shared singleton daemon

- `WEB-006` (in-progress): Shared singleton daemon and embedded web host; web/future TUI/CLI reuse one per-user/data-dir backend authority Obligations: `WEB-006-T01`..`T05`
- `WEB-007` (not-started): Full-width role-aligned transcript rows with action bars below each message Obligations: `WEB-007-T01`..`T05`
- `WEB-008` (not-started): Real edit/retry/regenerate/branch actions with preserved original history Obligations: `WEB-008-T01`..`T05`
- `WEB-009` (not-started): Collapsed safe reasoning summaries and tool activity, final answer separation and dedicated references Obligations: `WEB-009-T01`..`T05`
- `WEB-010` (not-started): Accessible WYSIWYG structured composer with model/effort, commands, mentions and capability affordances Obligations: `WEB-010-T01`..`T05`
- `WEB-011` (not-started): Real attachments, screenshots and local Library with bounded native provider mapping Obligations: `WEB-011-T01`..`T05`
- `WEB-012` (not-started): Plugin/app/tool chooser with native discovery, permissions, approvals and durable tool results Obligations: `WEB-012-T01`..`T05`
- `WEB-013` (not-started): Search/deep-research mode with source selection, plan, progress, steering and citations Obligations: `WEB-013-T01`..`T05`
- `WEB-014` (not-started): Pins, unified search, temporary chat and progressively paged long-history navigation Obligations: `WEB-014-T01`..`T05`
- `WEB-015` (not-started): Projects/workspaces, reusable context and memory/source disclosure Obligations: `WEB-015-T01`..`T05`
- `WEB-016` (not-started): Voice and dictation backed by a real native audio/realtime adapter Obligations: `WEB-016-T01`..`T05`
- `WEB-017` (not-started): Editable writing/code artifacts with safe preview/run/apply boundaries Obligations: `WEB-017-T01`..`T05`

### REQ-041 - Official provider compatibility and consent-based CLI authentication

- `PROV-015` (not-started): Provider auth profile model with explicit auth provenance and redacted storage Obligations: `PROV-015-T01`..`T05`
- `PROV-016` (not-started): Official OpenAI Codex OAuth connector with refresh, logout and account status Obligations: `PROV-016-T01`..`T05`
- `PROV-017` (not-started): Official Anthropic Claude Code OAuth connector with PKCE/loopback consent flow Obligations: `PROV-017-T01`..`T05`
- `PROV-018` (not-started): Consent-based import of Codex and Claude Code local credential files Obligations: `PROV-018-T01`..`T05`
- `PROV-019` (not-started): Documented provider request profiles with explicit identity and redacted diagnostics; no impersonation Obligations: `PROV-019-T01`..`T05`
- `PROV-020` (not-started): Auth connector commands and status surface Obligations: `PROV-020-T01`..`T05`
- `PROV-021` (not-started): Provider usage and limits telemetry with bounded status snapshots Obligations: `PROV-021-T01`..`T05`
- `PROV-022` (not-started): Auth storage hardening with keyring, 0600 fallback and atomic refresh persistence Obligations: `PROV-022-T01`..`T05`
- `PROV-023` (not-started): Versioned documentation-backed provider compatibility catalog Obligations: `PROV-023-T01`..`T05`
- `PROV-024` (not-started): Offline differential provider contract fixtures with secret redaction Obligations: `PROV-024-T01`..`T05`

### REQ-042 - TUI information panel and MCP management

- `UI-019` (not-started): Right-side TUI information panel with bounded live session, provider, context, MCP and warning metadata Obligations: `UI-019-T01`..`T05`
- `TOOL-016` (not-started): Bounded, source-attributed MCP catalog search and safe install metadata Obligations: `TOOL-016-T01`..`T05`
- `TOOL-017` (not-started): MCP selection and bulk lifecycle actions with confirmations and partial failure reporting Obligations: `TOOL-017-T01`..`T05`
- `TOOL-018` (not-started): Per-MCP power toggle and bounded lifecycle state persistence Obligations: `TOOL-018-T01`..`T05`
- `TOOL-019` (not-started): Disabled MCP servers and tools excluded from payloads and execution lookup Obligations: `TOOL-019-T01`..`T05`
- `TOOL-020` (not-started): MCP status events integrated into the TUI information panel Obligations: `TOOL-020-T01`..`T05`

### REQ-043 - Versioned sync event log with projector replay and persisted-versus-ephemeral part-event split

- `SYNC-001` (not-started): Versioned sync log with monotonic sequence allocation per aggregate, idempotent apply, projector registry, post-freeze definition rejection, deterministic replay Obligations: `SYNC-001-T01`..`T05`
- `SYNC-002` (not-started): Persisted message/part create-update-remove events versus ephemeral part-deltas; deltas never persist/replay; late foreign-key update warns without duplicating Obligations: `SYNC-002-T01`..`T05`

### REQ-044 - Bounded per-session runner with busy rejection, concurrent shell lane, and owned cancellation

- `RUN-001` (not-started): Per-session single normal runner: second prompt gets typed BusyError with zero side effects; shell lane runs concurrently; cancel reclaims task and removes idle runner Obligations: `RUN-001-T01`..`T05`

### REQ-045 - ACP v1 JSON-lines bridge over stdio with stated capability limits

- `ACP-001` (not-started): ACP v1 JSONL framing over caller-supplied byte streams: initialize, session new/load/prompt, modes/models/variants; malformed frame rejected without state change Obligations: `ACP-001-T01`..`T05`
- `ACP-002` (not-started): ACP text-file read/write with byte caps; unsupported full update stream, tool-call reporting, mode switch, auth, real terminal, full history restore return typed Unsupported Obligations: `ACP-002-T01`..`T05`

### REQ-046 - Workspace HTTP/WebSocket proxy and remote sync loop

- `WSX-001` (not-started): Workspace HTTP/WS proxy strips hop/routing headers, bounded queue-until-open, bridges /__workspace_ws, routes local versus remote by directory Obligations: `WSX-001-T01`..`T05`
- `WSX-002` (not-started): Remote workspace SSE sync loop with connected/connecting/disconnected/error statuses, bounded reconnect backoff, disposal closes queue/socket Obligations: `WSX-002-T01`..`T05`

### REQ-047 - Typed SDK client with server/process/TUI spawners and directory scoping

- `SDK-001` (not-started): Typed SDK client with directory header/query rewriting, abort/timeout propagation, typed decode errors, no process spawn Obligations: `SDK-001-T01`..`T05`
- `SDK-002` (not-started): SDK server/process/TUI spawners with owned child lifecycle, single owner, kill-on-drop, no detached tasks, bounded startup timeout Obligations: `SDK-002-T01`..`T05`

### REQ-048 - Headless run execution and session export

- `HEAD-001` (not-started): Headless run execution with inline/block renderers, attachment conversion, bounded output, typed exit codes, uncompressed prompt streaming Obligations: `HEAD-001-T01`..`T05`
- `HEAD-002` (not-started): Session export to caller-supplied writer as redacted JSON with explicit session id, no interactive picker in non-TTY, byte cap, no secret bytes Obligations: `HEAD-002-T01`..`T05`

- `AGENT-016` (accepted): Context fork - clone parent session into child with selective context injection Obligations: `AGENT-016-T01`..`T05`
- `AGENT-017` (accepted): Fresh context spawn - zero-context with explicit bundle Obligations: `AGENT-017-T01`..`T05`
- `AGENT-018` (accepted): Multi-provider routing - per-child model override, fallback chain Obligations: `AGENT-018-T01`..`T05`
- `AGENT-019` (accepted): Resume failed subagent - checkpoint restore, continue from failure Obligations: `AGENT-019-T01`..`T05`
- `AGENT-020` (accepted): Retry with exponential backoff - error classification, jitter, circuit breaker Obligations: `AGENT-020-T01`..`T05`
- `AGENT-021` (accepted): Change model mid-session - hot-swap, context adaptation Obligations: `AGENT-021-T01`..`T05`
- `AGENT-022` (accepted): Context compression - sliding window, summarize, hybrid strategies Obligations: `AGENT-022-T01`..`T05`
- `AGENT-023` (accepted): Structured handoff - portable state document with integrity Obligations: `AGENT-023-T01`..`T05`
- `AGENT-024` (accepted): Auto compression toggle - threshold, rate limit, quality target Obligations: `AGENT-024-T01`..`T05`
- `AGENT-025` (accepted): Dedicated subagent settings - max concurrent, model, retry, budget, pool Obligations: `AGENT-025-T01`..`T05`
- `AGENT-026` (accepted): Pool manager - maintain N agents, auto-replace, queue drain, scaling Obligations: `AGENT-026-T01`..`T05`
- `AGENT-027` (accepted): Observability - live dashboard, tokens, cost, latency percentiles, error rates Obligations: `AGENT-027-T01`..`T05`
- `AGENT-028` (accepted): Permission inheritance - narrow-only child permissions Obligations: `AGENT-028-T01`..`T05`
- `AGENT-029` (accepted): Output aggregation - merge, conflict resolution, dedup Obligations: `AGENT-029-T01`..`T05`
- `AGENT-030` (accepted): Cancellation and cleanup - cancel, reclaim, release locks Obligations: `AGENT-030-T01`..`T05`

## Story families (prefix index)

### AGENT (31 stories)

| Story | Requirements | Status | User story |
|---|---|---|---|
| `AGENT-001` | - | accepted | Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json |
| `AGENT-002` | `REQ-009` | accepted | TBD - see source audit |
| `AGENT-003` | `REQ-009`, `REQ-012` | accepted | TBD - see source audit |
| `AGENT-004` | `REQ-009`, `REQ-012` | accepted | TBD - see source audit |
| `AGENT-005` | `REQ-010` | accepted | TBD - see source audit |
| `AGENT-006` | `REQ-018` | accepted | TBD - see source audit |
| `AGENT-007` | `REQ-013` | accepted | TBD - see source audit |
| `AGENT-008` | `REQ-013` | accepted | TBD - see source audit |
| `AGENT-009` | - | accepted | Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json |
| `AGENT-010` | `REQ-009` | accepted | TBD - see source audit |
| `AGENT-011` | `REQ-014` | accepted | TBD - see source audit |
| `AGENT-012` | - | accepted | Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json |
| `AGENT-013` | - | accepted | Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json |
| `AGENT-014` | - | accepted | Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json |
| `AGENT-015` | `REQ-024` | accepted | TBD - see source audit |
| `AGENT-016` | `REQ-036` | accepted | Context fork spawn - clone parent session state into child with selective context |
| `AGENT-017` | `REQ-036` | accepted | Fresh context spawn - zero-context launch with explicit context bundle |
| `AGENT-018` | `REQ-036` | accepted | Multi-provider routing - spawn children on different providers/models |
| `AGENT-019` | `REQ-036` | accepted | Resume failed subagent - restore from checkpoint, continue from failure point |
| `AGENT-020` | `REQ-036` | accepted | Retry with exponential backoff - error classification, circuit breaker |
| `AGENT-021` | `REQ-036` | accepted | Change model mid-session - hot-swap without losing context |
| `AGENT-022` | `REQ-036` | accepted | Subagent context compression - automatic compaction near token limit |
| `AGENT-023` | `REQ-036` | accepted | Structured handoff - portable state document for agent-to-agent transfer |
| `AGENT-024` | `REQ-036` | accepted | Auto context compression toggle - settings-driven with thresholds |
| `AGENT-025` | `REQ-036` | accepted | Dedicated subagent settings - max concurrent, model, retry, budget, pool |
| `AGENT-026` | `REQ-036` | accepted | Subagent pool manager - maintain N agents, auto-replace, work queue drain |
| `AGENT-027` | `REQ-036` | accepted | Subagent observability - live dashboard, tokens, cost, latency, error rates |
| `AGENT-028` | `REQ-036` | accepted | Permission inheritance - child inherits parent, narrowing only |
| `AGENT-029` | `REQ-036` | accepted | Output aggregation - merge parallel results, conflict resolution, dedup |
| `AGENT-030` | `REQ-036` | accepted | Cancellation and cleanup - cancel children, reclaim resources, release locks |

### AUTO (7 stories)

| Story | Requirements | Status | User story |
|---|---|---|---|
| `AUTO-001` | `REQ-002` | accepted | TBD - see source audit |
| `AUTO-002` | `REQ-002`, `REQ-004` | accepted | TBD - see source audit |
| `AUTO-003` | `REQ-002` | accepted | TBD - see source audit |
| `AUTO-004` | `REQ-002` | in-progress | TBD - see source audit |
| `AUTO-005` | `REQ-002`, `REQ-004` | in-progress | TBD - see source audit |
| `AUTO-006` | `REQ-002` | in-progress | TBD - see source audit |

### BASE (8 stories)

| Story | Requirements | Status | User story |
|---|---|---|---|
| `BASE-001` | - | accepted | Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json |
| `BASE-002` | - | accepted | Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json |
| `BASE-003` | `REQ-001` | accepted | TBD - see source audit |
| `BASE-004` | `REQ-015` | accepted | TBD - see source audit |
| `BASE-005` | `REQ-015` | accepted | TBD - see source audit |
| `BASE-006` | `REQ-019`, `REQ-032` | accepted | TBD - see source audit |
| `BASE-007` | - | accepted | Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json |
| `BASE-008` | - | accepted | Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json |

### CAT (7 stories)

| Story | Requirements | Status | User story |
|---|---|---|---|
| `CAT-001` | `REQ-011` | accepted | TBD - see source audit |
| `CAT-002` | `REQ-011` | accepted | TBD - see source audit |
| `CAT-003` | - | accepted | Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json |
| `CAT-004` | `REQ-018` | accepted | TBD - see source audit |
| `CAT-005` | - | accepted | Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json |
| `CAT-006` | - | accepted | Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json |
| `CAT-007` | `REQ-011` | accepted | TBD - see source audit |

### DB (18 stories)

| Story | Requirements | Status | User story |
|---|---|---|---|
| `DB-001` | `REQ-033` | accepted | TBD - see source audit |
| `DB-002` | - | accepted | Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json |
| `DB-003` | `REQ-006`, `REQ-014` | accepted | TBD - see source audit |
| `DB-004` | - | accepted | Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json |
| `DB-005` | - | accepted | Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json |
| `DB-006` | `REQ-033` | accepted | TBD - see source audit |
| `DB-007` | - | accepted | Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json |
| `DB-008` | `REQ-033` | accepted | TBD - see source audit |
| `DB-009` | `REQ-033` | accepted | TBD - see source audit |
| `DB-010` | - | accepted | Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json |
| `DB-011` | - | accepted | Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json |
| `DB-012` | `REQ-034` | accepted | TBD - see source audit |
| `DB-013` | - | accepted | Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json |
| `DB-014` | - | accepted | Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json |
| `DB-015` | - | accepted | Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json |
| `DB-016` | `REQ-034` | accepted | TBD - see source audit |

### DISC (3 stories)

| Story | Requirements | Status | User story |
|---|---|---|---|
| `DISC-001` | `REQ-003` | accepted | TBD - see source audit |
| `DISC-008` | `REQ-035` | accepted | TBD - see source audit |
| `DISC-010` | `REQ-003` | accepted | TBD - see source audit |

### EXT (13 stories)

| Story | Requirements | Status | User story |
|---|---|---|---|
| `EXT-001` | `REQ-017` | in-progress | TBD - see source audit |
| `EXT-002` | `REQ-017` | in-progress | TBD - see source audit |
| `EXT-003` | - | accepted | Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json |
| `EXT-004` | - | in-progress | Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json |
| `EXT-005` | `REQ-005` | in-progress | TBD - see source audit |
| `EXT-006` | - | in-progress | Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json |
| `EXT-007` | - | accepted | Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json |
| `EXT-008` | `REQ-020` | in-progress | TBD - see source audit |
| `EXT-009` | `REQ-005` | in-progress | TBD - see source audit |
| `EXT-010` | - | in-progress | Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json |
| `EXT-011` | - | in-progress | Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json |
| `EXT-012` | `REQ-005` | in-progress | TBD - see source audit |

### INT (10 stories)

| Story | Requirements | Status | User story |
|---|---|---|---|
| `INT-001` | - | in-progress | Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json |
| `INT-002` | - | in-progress | Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json |
| `INT-003` | - | in-progress | Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json |
| `INT-004` | - | accepted | Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json |
| `INT-005` | - | in-progress | Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json |
| `INT-006` | - | in-progress | Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json |
| `INT-007` | - | in-progress | Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json |
| `INT-008` | - | accepted | Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json |
| `INT-009` | - | in-progress | Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json |
| `INT-010` | - | in-progress | Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json |

### OPS (10 stories)

| Story | Requirements | Status | User story |
|---|---|---|---|
| `OPS-001` | `REQ-001`, `REQ-024`, `REQ-032` | in-progress | TBD - see source audit |
| `OPS-002` | - | in-progress | Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json |
| `OPS-003` | - | in-progress | Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json |
| `OPS-004` | - | in-progress | Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json |
| `OPS-005` | - | in-progress | Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json |
| `OPS-006` | - | in-progress | Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json |
| `OPS-007` | `REQ-001`, `REQ-024` | in-progress | TBD - see source audit |
| `OPS-008` | - | in-progress | Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json |
| `OPS-009` | - | in-progress | Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json |

### PROV (24 stories)

| Story | Requirements | Status | User story |
|---|---|---|---|
| `PROV-001` | - | accepted | Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json |
| `PROV-002` | - | accepted | Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json |
| `PROV-003` | - | accepted | Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json |
| `PROV-004` | - | accepted | Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json |
| `PROV-005` | - | accepted | Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json |
| `PROV-006` | - | accepted | Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json |
| `PROV-007` | - | accepted | Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json |
| `PROV-008` | - | accepted | Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json |
| `PROV-009` | - | accepted | Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json |
| `PROV-010` | - | accepted | Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json |
| `PROV-011` | `REQ-010` | accepted | TBD - see source audit |
| `PROV-012` | - | accepted | Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json |
| `PROV-015` | `REQ-041` | not-started | Provider auth profile model with explicit auth provenance and redacted storage |
| `PROV-016` | `REQ-041` | not-started | Official OpenAI Codex OAuth connector |
| `PROV-017` | `REQ-041` | not-started | Official Anthropic Claude Code OAuth connector |
| `PROV-018` | `REQ-041` | not-started | Consent-based Codex and Claude Code local credential import |
| `PROV-019` | `REQ-041` | not-started | Documented provider request profiles without impersonation |
| `PROV-020` | `REQ-041` | not-started | Auth connector commands and status surface |
| `PROV-021` | `REQ-041` | not-started | Provider usage and limits telemetry |
| `PROV-022` | `REQ-041` | not-started | Auth storage hardening and atomic refresh persistence |
| `PROV-023` | `REQ-041` | not-started | Versioned provider compatibility catalog |
| `PROV-024` | `REQ-041` | not-started | Offline differential provider contract fixtures |

### REL (4 stories)

| Story | Requirements | Status | User story |
|---|---|---|---|
| `REL-001` | `REQ-003` | in-progress | TBD - see source audit |
| `REL-002` | `REQ-004`, `REQ-035` | in-progress | TBD - see source audit |
| `REL-003` | `REQ-035` | in-progress | TBD - see source audit |

### ROUTE (12 stories)

| Story | Requirements | Status | User story |
|---|---|---|---|
| `ROUTE-001` | `REQ-022` | accepted | TBD - see source audit |
| `ROUTE-002` | `REQ-022` | accepted | TBD - see source audit |
| `ROUTE-003` | - | accepted | Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json |
| `ROUTE-004` | - | accepted | Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json |
| `ROUTE-005` | `REQ-022` | accepted | TBD - see source audit |
| `ROUTE-006` | - | accepted | Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json |
| `ROUTE-007` | - | accepted | Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json |
| `ROUTE-008` | `REQ-016` | accepted | TBD - see source audit |
| `ROUTE-009` | - | accepted | Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json |
| `ROUTE-010` | - | accepted | Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json |
| `ROUTE-011` | `REQ-022` | accepted | TBD - see source audit |

### SEC (19 stories)

| Story | Requirements | Status | User story |
|---|---|---|---|
| `SEC-001` | `REQ-021`, `REQ-030` | accepted | TBD - see source audit |
| `SEC-002` | `REQ-025` | accepted | TBD - see source audit |
| `SEC-003` | `REQ-021`, `REQ-029`, `REQ-030` | accepted | TBD - see source audit |
| `SEC-004` | `REQ-027` | accepted | TBD - see source audit |
| `SEC-005` | `REQ-027` | accepted | TBD - see source audit |
| `SEC-006` | `REQ-028`, `REQ-029` | accepted | TBD - see source audit |
| `SEC-007` | `REQ-031` | accepted | TBD - see source audit |
| `SEC-008` | - | accepted | Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json |
| `SEC-009` | - | accepted | Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json |
| `SEC-010` | `REQ-020` | accepted | TBD - see source audit |
| `SEC-011` | `REQ-020` | accepted | TBD - see source audit |
| `SEC-012` | `REQ-025` | accepted | TBD - see source audit |
| `SEC-013` | `REQ-026` | accepted | TBD - see source audit |
| `SEC-014` | `REQ-027` | accepted | TBD - see source audit |
| `SEC-016` | `REQ-025` | accepted | TBD - see source audit |
| `SEC-017` | `REQ-030`, `REQ-035` | accepted | TBD - see source audit |

### SESS (20 stories)

| Story | Requirements | Status | User story |
|---|---|---|---|
| `SESS-001` | `REQ-006` | accepted | TBD - see source audit |
| `SESS-002` | `REQ-023` | accepted | TBD - see source audit |
| `SESS-003` | - | accepted | Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json |
| `SESS-004` | - | accepted | Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json |
| `SESS-005` | - | accepted | Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json |
| `SESS-006` | - | accepted | Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json |
| `SESS-007` | - | accepted | Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json |
| `SESS-008` | - | accepted | Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json |
| `SESS-009` | - | accepted | Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json |
| `SESS-010` | - | accepted | Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json |
| `SESS-011` | `REQ-008` | accepted | TBD - see source audit |
| `SESS-012` | - | accepted | Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json |
| `SESS-013` | - | accepted | Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json |
| `SESS-014` | - | accepted | Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json |
| `SESS-015` | - | accepted | Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json |
| `SESS-016` | - | accepted | Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json |
| `SESS-017` | `REQ-006` | accepted | TBD - see source audit |

### SHARE (5 stories)

| Story | Requirements | Status | User story |
|---|---|---|---|
| `SHARE-001` | `REQ-007` | in-progress | TBD - see source audit |
| `SHARE-002` | - | in-progress | Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json |
| `SHARE-003` | - | in-progress | Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json |
| `SHARE-004` | `REQ-007` | in-progress | TBD - see source audit |
| `SHARE-005` | `REQ-007` | in-progress | TBD - see source audit |

### TOOL (20 stories)

| Story | Requirements | Status | User story |
|---|---|---|---|
| `TOOL-001` | - | accepted | Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json |
| `TOOL-002` | - | accepted | Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json |
| `TOOL-003` | - | accepted | Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json |
| `TOOL-004` | - | accepted | Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json |
| `TOOL-005` | - | accepted | Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json |
| `TOOL-006` | `REQ-037` | accepted | Typed tool contract and capability-bound tool invocation |
| `TOOL-007` | - | accepted | Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json |
| `TOOL-008` | - | accepted | Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json |
| `TOOL-009` | `REQ-037` | accepted | Tool diagnostics and bounded execution reporting |
| `TOOL-010` | - | accepted | Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json |
| `TOOL-011` | - | accepted | Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json |
| `TOOL-012` | - | accepted | Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json |
| `TOOL-013` | - | accepted | Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json |
| `TOOL-014` | - | accepted | Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json |
| `TOOL-015` | `REQ-037` | accepted | Tool allow-list and review discipline |
| `TOOL-016` | `REQ-042` | not-started | Bounded, source-attributed MCP catalog search and safe install metadata |
| `TOOL-017` | `REQ-042` | not-started | MCP selection and bulk lifecycle actions with confirmations and partial failure reporting |
| `TOOL-018` | `REQ-042` | not-started | Per-MCP power toggle and bounded lifecycle state persistence |
| `TOOL-019` | `REQ-042` | not-started | Disabled MCP servers and tools excluded from payloads and execution lookup |
| `TOOL-020` | `REQ-042` | not-started | MCP status events integrated into the TUI information panel |

### UI (19 stories)

| Story | Requirements | Status | User story |
|---|---|---|---|
| `UI-001` | - | accepted | Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json |
| `UI-002` | - | accepted | Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json |
| `UI-003` | `REQ-023` | accepted | TBD - see source audit |
| `UI-004` | `REQ-008` | accepted | TBD - see source audit |
| `UI-005` | `REQ-013` | accepted | TBD - see source audit |
| `UI-006` | `REQ-016` | accepted | TBD - see source audit |
| `UI-007` | `REQ-018` | accepted | TBD - see source audit |
| `UI-008` | `REQ-021` | accepted | TBD - see source audit |
| `UI-009` | `REQ-019` | accepted | TBD - see source audit |
| `UI-010` | `REQ-017` | accepted | TBD - see source audit |
| `UI-011` | `REQ-029`, `REQ-032` | accepted | TBD - see source audit |
| `UI-012` | `REQ-005` | accepted | TBD - see source audit |
| `UI-013` | - | accepted | Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json |
| `UI-019` | `REQ-042` | not-started | Right-side TUI information panel with bounded live session, provider, context, MCP and warning metadata |

### WEB (17 stories)

| Story | Requirements | Status | User story |
|---|---|---|---|
| `WEB-001` | - | in-progress | Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json |
| `WEB-002` | - | in-progress | Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json |
| `WEB-003` | - | in-progress | Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json |
| `WEB-004` | - | in-progress | Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json |
| `WEB-005` | - | in-progress | Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json |
| `WEB-006` | `REQ-040` | in-progress | Shared singleton daemon and embedded web host - web, future TUI and CLI use one per-user/data-dir native backend authority without a separate web-only serve process |
| `WEB-007` | `REQ-040` | not-started | Full-width role-aligned transcript rows with request/response action bars below each message and honest capability-gated actions |
| `WEB-008` | `REQ-040` | not-started | Real edit, retry, regenerate and branch/fork actions preserve original conversation history and expose branch navigation |
| `WEB-009` | `REQ-040` | not-started | Structured assistant turns render collapsed provider reasoning summaries and tool activity separately from final answer text with a dedicated references section; raw hidden chain-of-thought is never exposed |
| `WEB-010` | `REQ-040` | not-started | Accessible WYSIWYG structured chat composer with model and effort controls, slash commands, mentions, attachment/tool/plugin affordances, drafts and stop/queue/steer state |
| `WEB-011` | `REQ-040` | not-started | Real bounded files, images and screenshots through picker, paste, drag/drop and a searchable local Library with native attachment-provider mapping |
| `WEB-012` | `REQ-040` | not-started | Composer plugin, app and tool discovery/selection backed by real native capabilities with permission approvals and durable tool-call results |
| `WEB-013` | `REQ-040` | not-started | Search and deep-research mode with real source/tool selection, reviewable plan, progress, steering, cancellation and cited final results |
| `WEB-014` | `REQ-040` | not-started | Chat navigation parity with pins, unified history search, archive/share, temporary chat, progressive long-history paging and keyboard navigation |
| `WEB-015` | `REQ-040` | not-started | Projects and workspaces group chats, files and reusable context with memory/context inspection and disclosure of sources used |
| `WEB-016` | `REQ-040` | not-started | Voice and dictation with explicit microphone state, visible transcript and a real native audio/realtime adapter |
| `WEB-017` | `REQ-040` | not-started | Editable writing and code artifacts with copy/edit/undo/preview and explicitly authorized safe run/apply actions |

## Stories without a direct requirement link (96)

| `TOOL-006` | `REQ-037` | accepted | Typed tool contract via buildTool factory - schema plus permission plus exec plus render in one definition |
| `TOOL-007` | `REQ-017` | accepted | Skill-gated approvals - skill invocation passes the same policy gate as tools |
| `TOOL-009` | `REQ-037` | accepted | ToolSearch discovery - just-in-time tool listing to keep lean context |
| `TOOL-010` | `REQ-021`, `REQ-026` | accepted | request_permissions mid-turn tool - agent asks for a named capability with scope, rate-limited |
| `TOOL-015` | `REQ-037` | accepted | MCP tool own policy gate plus elicitation path with timeout and cancel |
| `SEC-018` | `REQ-025` | accepted | Dangerous shell-pattern denylist stripped at auto-entry - cross-platform code-exec block plus matcher tests |
| `SEC-019` | `REQ-025`, `REQ-030` | accepted | Execpolicy prefix rules - declarative path and command allowlist, longest-match evaluated before tools |
| `SEC-020` | `REQ-020` | accepted | Bounded hook bus - max 100 pending with shift-drop, always-emit allowlist, SSRF guard on HTTP hooks |
| `AUTO-007` | `REQ-002`, `REQ-012` | accepted | Turn submission state machine - submit to running to interrupted or complete as typed transitions with cancel cleanup |
| `DB-017` | `REQ-006`, `REQ-033` | accepted | Dual rollout record - append-only JSONL is truth, sqlite holds queryable snapshot, recorder owns order |
| `DB-018` | `REQ-033` | accepted | Content-addressed transcript dedupe - repeated prompts and tool schemas stored by canonical hash, turns reference ids |
| `PROV-013` | `REQ-038` | accepted | Provider-boundary tap with redaction - uid-keyed structured records at request, response, stream-final, error seam |
| `PROV-014` | `REQ-038` | accepted | Debug export bundle - one redacted JSONL with session meta, deduped defs, turns, timings, errors; offline renderer |
| `OPS-010` | `REQ-037` | accepted | Doctor diagnostics command - checks auth, connectivity, tools, MCP; reports failures without log spelunking |
| `EXT-013` | `REQ-017`, `REQ-027` | accepted | Skill safe extraction - bundled files materialize once, owner-only 0600 O_EXCL O_NOFOLLOW, traversal rejected |
| `SESS-018` | `REQ-008` | accepted | Fork boundary is typed - branch copy only on fork, cache cleared at boundary |
| `SESS-019` | `REQ-008` | accepted | Revert is free pointer move distinct from truncating rollback with separate API names |
| `SESS-020` | `REQ-006` | accepted | Auto-compact thresholds plus warning states plus 3-strike circuit breaker with env kill-switches |
| `ROUTE-012` | `REQ-018` | accepted | Cheap-model chores routing - quota, topic, title, summarize go to cheapest capable model, fail-open |
| `AGENT-031` | `REQ-013` | accepted | Plan mode plus structured review child - plan permission mode with Enter and Exit tools, review returns machine-checkable findings |
| `REL-004` | `REQ-016` | accepted | Per-turn cost counters - input, output, cache-read, cache-create, durations, lines changed, web-search count, unknown-cost flag |
| `UI-014` | `REQ-016`, `REQ-018` | accepted | TUI composer - multiline editor, Enter sends, Shift+Enter newline, Ctrl+J fallback, send button, draft survives interrupt, queue-while-busy |
| `UI-015` | `REQ-016` | accepted | TUI status bar - clickable model item opens provider-aware switcher, clickable context item opens context detail view; keyboard fallback |
| `UI-016` | `REQ-016` | accepted | TUI context detail view - per-source token breakdown with cache-read vs fresh flags, largest blocks first, truncation markers |
| `UI-017` | `REQ-006`, `REQ-016` | accepted | TUI /memory viewer - list loaded memory files with path bytes tokens cached flag, unload with prefix-cache cost warning, reload path |
| `UI-018` | `REQ-019` | accepted | TUI keybindings help - footer hints plus /keybindings parity, configurable composer submit keymap |
These stories carry `requirementIds: []` in `ralph.json`. They are still mandatory backlog (typically DISC-002 surface-extraction discoveries or not-yet-reconciled scope). Do not treat absence of a requirement link as optional. Task cards in `tasks/` and `feature-ledger.json` may add ownership detail.

- `AGENT-001` (accepted): Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json
- `AGENT-009` (accepted): Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json
- `AGENT-012` (accepted): Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json
- `AGENT-013` (accepted): Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json
- `AGENT-014` (accepted): Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json
- `BASE-001` (accepted): Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json
- `BASE-002` (accepted): Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json
- `BASE-007` (accepted): Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json
- `BASE-008` (accepted): Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json
- `CAT-003` (accepted): Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json
- `CAT-005` (accepted): Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json
- `CAT-006` (accepted): Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json
- `DB-002` (accepted): Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json
- `DB-004` (accepted): Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json
- `DB-005` (accepted): Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json
- `DB-007` (accepted): Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json
- `DB-010` (accepted): Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json
- `DB-011` (accepted): Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json
- `DB-013` (accepted): Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json
- `DB-014` (accepted): Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json
- `DB-015` (accepted): Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json
- `EXT-003` (accepted): Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json
- `EXT-004` (in-progress): Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json
- `EXT-006` (in-progress): Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json
- `EXT-007` (accepted): Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json
- `EXT-010` (in-progress): Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json
- `EXT-011` (in-progress): Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json
- `INT-001` (in-progress): Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json
- `INT-002` (in-progress): Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json
- `INT-003` (in-progress): Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json
- `INT-004` (accepted): Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json
- `INT-005` (in-progress): Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json
- `INT-006` (in-progress): Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json
- `INT-007` (in-progress): Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json
- `INT-008` (accepted): Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json
- `INT-009` (in-progress): Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json
- `INT-010` (in-progress): Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json
- `OPS-002` (in-progress): Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json
- `OPS-003` (in-progress): Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json
- `OPS-004` (in-progress): Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json
- `OPS-005` (in-progress): Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json
- `OPS-006` (in-progress): Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json
- `OPS-008` (in-progress): Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json
- `OPS-009` (in-progress): Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json
- `PROV-001` (accepted): Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json
- `PROV-002` (accepted): Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json
- `PROV-003` (accepted): Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json
- `PROV-004` (accepted): Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json
- `PROV-005` (accepted): Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json
- `PROV-006` (accepted): Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json
- `PROV-007` (accepted): Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json
- `PROV-008` (accepted): Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json
- `PROV-009` (accepted): Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json
- `PROV-010` (accepted): Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json
- `PROV-012` (accepted): Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json
- `ROUTE-003` (accepted): Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json
- `ROUTE-004` (accepted): Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json
- `ROUTE-006` (accepted): Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json
- `ROUTE-007` (accepted): Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json
- `ROUTE-009` (accepted): Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json
- `ROUTE-010` (accepted): Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json
- `SEC-008` (accepted): Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json
- `SEC-009` (accepted): Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json
- `SESS-003` (accepted): Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json
- `SESS-004` (accepted): Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json
- `SESS-005` (accepted): Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json
- `SESS-006` (accepted): Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json
- `SESS-007` (accepted): Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json
- `SESS-008` (accepted): Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json
- `SESS-009` (accepted): Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json
- `SESS-010` (accepted): Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json
- `SESS-012` (accepted): Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json
- `SESS-013` (accepted): Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json
- `SESS-014` (accepted): Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json
- `SESS-015` (accepted): Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json
- `SESS-016` (accepted): Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json
- `SHARE-002` (in-progress): Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json
- `SHARE-003` (in-progress): Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json
- `TOOL-001` (accepted): Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json
- `TOOL-002` (accepted): Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json
- `TOOL-003` (accepted): Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json
- `TOOL-004` (accepted): Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json
- `TOOL-005` (accepted): Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json
- `TOOL-008` (accepted): Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json
- `TOOL-011` (accepted): Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json
- `TOOL-012` (accepted): Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json
- `TOOL-013` (accepted): Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json
- `TOOL-014` (accepted): Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json
- `TOOL-006` (accepted): Typed tool contract via buildTool factory Obligations: `TOOL-006-T01`..`T05`
- `TOOL-007` (accepted): Skill-gated approvals Obligations: `TOOL-007-T01`..`T05`
- `TOOL-009` (accepted): ToolSearch discovery for lean context Obligations: `TOOL-009-T01`..`T05`
- `TOOL-010` (accepted): request_permissions mid-turn tool Obligations: `TOOL-010-T01`..`T05`
- `TOOL-015` (accepted): MCP tool policy gate plus elicitation Obligations: `TOOL-015-T01`..`T05`
- `UI-001` (accepted): Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json
- `UI-002` (accepted): Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json
- `UI-013` (accepted): Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json
- `WEB-001` (in-progress): Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json
- `WEB-002` (in-progress): Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json
- `WEB-003` (in-progress): Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json
- `WEB-004` (in-progress): Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json
- `WEB-005` (in-progress): Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json

### SYNC (2 stories)

| Story | Requirements | Status | User story |
|---|---|---|---|
| `SYNC-001` | `REQ-043` | not-started | Versioned sync log with monotonic sequence allocation per aggregate, idempotent apply, projector registry, post-freeze definition rejection, deterministic replay |
| `SYNC-002` | `REQ-043` | not-started | Persisted message/part create-update-remove events versus ephemeral part-deltas; deltas never persist/replay; late foreign-key update warns without duplicating |

### RUN (1 story)

| Story | Requirements | Status | User story |
|---|---|---|---|
| `RUN-001` | `REQ-044` | not-started | Per-session single normal runner: second prompt gets typed BusyError with zero side effects; shell lane runs concurrently; cancel reclaims task and removes idle runner |

### ACP (2 stories)

| Story | Requirements | Status | User story |
|---|---|---|---|
| `ACP-001` | `REQ-045` | not-started | ACP v1 JSONL framing over caller-supplied byte streams: initialize, session new/load/prompt, modes/models/variants; malformed frame rejected without state change |
| `ACP-002` | `REQ-045` | not-started | ACP text-file read/write with byte caps; unsupported full update stream, tool-call reporting, mode switch, auth, real terminal, full history restore return typed Unsupported |

### WSX (2 stories)

| Story | Requirements | Status | User story |
|---|---|---|---|
| `WSX-001` | `REQ-046` | not-started | Workspace HTTP/WS proxy strips hop/routing headers, bounded queue-until-open, bridges /__workspace_ws, routes local versus remote by directory |
| `WSX-002` | `REQ-046` | not-started | Remote workspace SSE sync loop with connected/connecting/disconnected/error statuses, bounded reconnect backoff, disposal closes queue/socket |

### SDK (2 stories)

| Story | Requirements | Status | User story |
|---|---|---|---|
| `SDK-001` | `REQ-047` | not-started | Typed SDK client with directory header/query rewriting, abort/timeout propagation, typed decode errors, no process spawn |
| `SDK-002` | `REQ-047` | not-started | SDK server/process/TUI spawners with owned child lifecycle, single owner, kill-on-drop, no detached tasks, bounded startup timeout |

### HEAD (2 stories)

| Story | Requirements | Status | User story |
|---|---|---|---|
| `HEAD-001` | `REQ-048` | not-started | Headless run execution with inline/block renderers, attachment conversion, bounded output, typed exit codes, uncompressed prompt streaming |
| `HEAD-002` | `REQ-048` | not-started | Session export to caller-supplied writer as redacted JSON with explicit session id, no interactive picker in non-TTY, byte cap, no secret bytes |
