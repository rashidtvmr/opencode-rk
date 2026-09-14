# FEATURES - navigable feature index

Generated from `ralph.json` (canonical extended plan) and `requirements/user-requirements.json`.
Every story is mandatory to the declared full release even when its feature is off by default at runtime (PLAN.md section 1).
`prd.json` is the flat conventional Ralph export of the same data. Dependency eligibility is decided by `ralph.json` plus the milestone ranks in PLAN.md section 4; see `tools/plan_model.py`.

Stories: 219. Requirements: 38. Test obligations: 1095.

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

## Requirement detail

### REQ-001 - Rust plus Tokio and maximum practical resource savings

- `BASE-003` (accepted): TBD - see source audit Obligations: `BASE-003-T01`, `BASE-003-T02`, `BASE-003-T03`, `BASE-003-T04`, `BASE-003-T05`
- `OPS-001` (not-started): TBD - see source audit Obligations: `OPS-001-T01`, `OPS-001-T02`, `OPS-001-T03`, `OPS-001-T04`, `OPS-001-T05`
- `OPS-007` (not-started): TBD - see source audit Obligations: `OPS-007-T01`, `OPS-007-T02`, `OPS-007-T03`, `OPS-007-T04`, `OPS-007-T05`

### REQ-002 - Plan file Ralph JSON independent slices and autonomous loop

- `AUTO-001` (accepted): TBD - see source audit Obligations: `AUTO-001-T01`, `AUTO-001-T02`, `AUTO-001-T03`, `AUTO-001-T04`, `AUTO-001-T05`
- `AUTO-002` (accepted): TBD - see source audit Obligations: `AUTO-002-T01`, `AUTO-002-T02`, `AUTO-002-T03`, `AUTO-002-T04`, `AUTO-002-T05`
- `AUTO-003` (in-progress): TBD - see source audit Obligations: `AUTO-003-T01`, `AUTO-003-T02`, `AUTO-003-T03`, `AUTO-003-T04`, `AUTO-003-T05`
- `AUTO-004` (in-progress): TBD - see source audit Obligations: `AUTO-004-T01`, `AUTO-004-T02`, `AUTO-004-T03`, `AUTO-004-T04`, `AUTO-004-T05`
- `AUTO-005` (in-progress): TBD - see source audit Obligations: `AUTO-005-T01`, `AUTO-005-T02`, `AUTO-005-T03`, `AUTO-005-T04`, `AUTO-005-T05`
- `AUTO-006` (in-progress): TBD - see source audit Obligations: `AUTO-006-T01`, `AUTO-006-T02`, `AUTO-006-T03`, `AUTO-006-T04`, `AUTO-006-T05`
- `AUTO-007` (in-progress): Turn submission state machine Obligations: `AUTO-007-T01`..`T05`

### REQ-003 - Every OpenCode V2 feature accounted for

- `DISC-001` (accepted): TBD - see source audit Obligations: `DISC-001-T01`, `DISC-001-T02`, `DISC-001-T03`, `DISC-001-T04`, `DISC-001-T05`
- `DISC-010` (accepted): TBD - see source audit Obligations: `DISC-010-T01`, `DISC-010-T02`, `DISC-010-T03`, `DISC-010-T04`, `DISC-010-T05`
- `REL-001` (not-started): TBD - see source audit Obligations: `REL-001-T01`, `REL-001-T02`, `REL-001-T03`, `REL-001-T04`, `REL-001-T05`

### REQ-004 - Strict TDD and independent verification

- `AUTO-002` (accepted): TBD - see source audit Obligations: `AUTO-002-T01`, `AUTO-002-T02`, `AUTO-002-T03`, `AUTO-002-T04`, `AUTO-002-T05`
- `AUTO-005` (in-progress): TBD - see source audit Obligations: `AUTO-005-T01`, `AUTO-005-T02`, `AUTO-005-T03`, `AUTO-005-T04`, `AUTO-005-T05`
- `REL-002` (not-started): TBD - see source audit Obligations: `REL-002-T01`, `REL-002-T02`, `REL-002-T03`, `REL-002-T04`, `REL-002-T05`

### REQ-005 - OpenCode V2 plugins including UI behavior

- `EXT-005` (in-progress): TBD - see source audit Obligations: `EXT-005-T01`, `EXT-005-T02`, `EXT-005-T03`, `EXT-005-T04`, `EXT-005-T05`
- `EXT-009` (in-progress): TBD - see source audit Obligations: `EXT-009-T01`, `EXT-009-T02`, `EXT-009-T03`, `EXT-009-T04`, `EXT-009-T05`
- `EXT-012` (in-progress): TBD - see source audit Obligations: `EXT-012-T01`, `EXT-012-T02`, `EXT-012-T03`, `EXT-012-T04`, `EXT-012-T05`
- `UI-012` (not-started): TBD - see source audit Obligations: `UI-012-T01`, `UI-012-T02`, `UI-012-T03`, `UI-012-T04`, `UI-012-T05`

### REQ-006 - Session management and persistent history

- `SESS-001` (accepted): TBD - see source audit Obligations: `SESS-001-T01`, `SESS-001-T02`, `SESS-001-T03`, `SESS-001-T04`, `SESS-001-T05`
- `DB-003` (accepted): TBD - see source audit Obligations: `DB-003-T01`, `DB-003-T02`, `DB-003-T03`, `DB-003-T04`, `DB-003-T05`
- `SESS-017` (accepted): TBD - see source audit Obligations: `SESS-017-T01`, `SESS-017-T02`, `SESS-017-T03`, `SESS-017-T04`, `SESS-017-T05`
- `DB-017` (accepted): Dual rollout record JSONL plus sqlite Obligations: `DB-017-T01`..`T05`
- `SESS-020` (in-progress): Auto-compact thresholds plus breaker Obligations: `SESS-020-T01`..`T05`
- `UI-017` (not-started): TUI /memory viewer Obligations: `UI-017-T01`..`T05`

### REQ-007 - Session sharing

- `SHARE-001` (not-started): TBD - see source audit Obligations: `SHARE-001-T01`, `SHARE-001-T02`, `SHARE-001-T03`, `SHARE-001-T04`, `SHARE-001-T05`
- `SHARE-004` (not-started): TBD - see source audit Obligations: `SHARE-004-T01`, `SHARE-004-T02`, `SHARE-004-T03`, `SHARE-004-T04`, `SHARE-004-T05`
- `SHARE-005` (not-started): TBD - see source audit Obligations: `SHARE-005-T01`, `SHARE-005-T02`, `SHARE-005-T03`, `SHARE-005-T04`, `SHARE-005-T05`

### REQ-008 - Session forking

- `SESS-011` (accepted): TBD - see source audit Obligations: `SESS-011-T01`, `SESS-011-T02`, `SESS-011-T03`, `SESS-011-T04`, `SESS-011-T05`
- `UI-004` (not-started): TBD - see source audit Obligations: `UI-004-T01`, `UI-004-T02`, `UI-004-T03`, `UI-004-T04`, `UI-004-T05`
- `SESS-018` (accepted): Typed fork boundary Obligations: `SESS-018-T01`..`T05`
- `SESS-019` (in-progress): Revert vs rollback split Obligations: `SESS-019-T01`..`T05`

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
- `UI-005` (not-started): TBD - see source audit Obligations: `UI-005-T01`, `UI-005-T02`, `UI-005-T03`, `UI-005-T04`, `UI-005-T05`
- `AGENT-031` (accepted): Plan mode plus structured review child Obligations: `AGENT-031-T01`..`T05`

### REQ-014 - Subagent context and messages in DB

- `AGENT-011` (accepted): TBD - see source audit Obligations: `AGENT-011-T01`, `AGENT-011-T02`, `AGENT-011-T03`, `AGENT-011-T04`, `AGENT-011-T05`
- `DB-003` (accepted): TBD - see source audit Obligations: `DB-003-T01`, `DB-003-T02`, `DB-003-T03`, `DB-003-T04`, `DB-003-T05`

### REQ-015 - Singleton supports many application clients

- `BASE-004` (accepted): TBD - see source audit Obligations: `BASE-004-T01`, `BASE-004-T02`, `BASE-004-T03`, `BASE-004-T04`, `BASE-004-T05`
- `BASE-005` (accepted): TBD - see source audit Obligations: `BASE-005-T01`, `BASE-005-T02`, `BASE-005-T03`, `BASE-005-T04`, `BASE-005-T05`

### REQ-016 - Status panel

- `UI-006` (not-started): TBD - see source audit Obligations: `UI-006-T01`, `UI-006-T02`, `UI-006-T03`, `UI-006-T04`, `UI-006-T05`
- `ROUTE-008` (not-started): TBD - see source audit Obligations: `ROUTE-008-T01`, `ROUTE-008-T02`, `ROUTE-008-T03`, `ROUTE-008-T04`, `ROUTE-008-T05`
- `REL-004` (not-started): Per-turn cost counters Obligations: `REL-004-T01`..`T05`
- `UI-014` (not-started): TUI composer Obligations: `UI-014-T01`..`T05`
- `UI-015` (not-started): TUI status bar Obligations: `UI-015-T01`..`T05`
- `UI-016` (not-started): TUI context detail view Obligations: `UI-016-T01`..`T05`
- `UI-017` (not-started): TUI /memory viewer Obligations: `UI-017-T01`..`T05`

### REQ-017 - Skills plugins and custom slash commands

- `EXT-001` (in-progress): TBD - see source audit Obligations: `EXT-001-T01`, `EXT-001-T02`, `EXT-001-T03`, `EXT-001-T04`, `EXT-001-T05`
- `EXT-002` (in-progress): TBD - see source audit Obligations: `EXT-002-T01`, `EXT-002-T02`, `EXT-002-T03`, `EXT-002-T04`, `EXT-002-T05`
- `UI-010` (not-started): TBD - see source audit Obligations: `UI-010-T01`, `UI-010-T02`, `UI-010-T03`, `UI-010-T04`, `UI-010-T05`
- `EXT-013` (in-progress): Skill safe extraction Obligations: `EXT-013-T01`..`T05`
- `TOOL-007` (accepted): Skill-gated approvals Obligations: `TOOL-007-T01`..`T05`

### REQ-018 - Independent main and child effort levels

- `CAT-004` (accepted): TBD - see source audit Obligations: `CAT-004-T01`, `CAT-004-T02`, `CAT-004-T03`, `CAT-004-T04`, `CAT-004-T05`
- `AGENT-006` (accepted): TBD - see source audit Obligations: `AGENT-006-T01`, `AGENT-006-T02`, `AGENT-006-T03`, `AGENT-006-T04`, `AGENT-006-T05`
- `UI-007` (not-started): TBD - see source audit Obligations: `UI-007-T01`, `UI-007-T02`, `UI-007-T03`, `UI-007-T04`, `UI-007-T05`
- `ROUTE-012` (in-progress): Cheap-model chores routing Obligations: `ROUTE-012-T01`..`T05`

### REQ-019 - Themes and common settings

- `UI-009` (not-started): TBD - see source audit Obligations: `UI-009-T01`, `UI-009-T02`, `UI-009-T03`, `UI-009-T04`, `UI-009-T05`
- `BASE-006` (accepted): TBD - see source audit Obligations: `BASE-006-T01`, `BASE-006-T02`, `BASE-006-T03`, `BASE-006-T04`, `BASE-006-T05`
- `UI-018` (not-started): TUI keybindings help Obligations: `UI-018-T01`..`T05`

### REQ-020 - Pre-tool and post-tool hooks

- `SEC-010` (accepted): TBD - see source audit Obligations: `SEC-010-T01`, `SEC-010-T02`, `SEC-010-T03`, `SEC-010-T04`, `SEC-010-T05`
- `SEC-011` (accepted): TBD - see source audit Obligations: `SEC-011-T01`, `SEC-011-T02`, `SEC-011-T03`, `SEC-011-T04`, `SEC-011-T05`
- `SEC-020` (accepted): Bounded hook bus Obligations: `SEC-020-T01`..`T05`
- `EXT-008` (in-progress): TBD - see source audit Obligations: `EXT-008-T01`, `EXT-008-T02`, `EXT-008-T03`, `EXT-008-T04`, `EXT-008-T05`

### REQ-021 - Permissions and mandatory human-in-the-loop

- `SEC-001` (accepted): TBD - see source audit Obligations: `SEC-001-T01`, `SEC-001-T02`, `SEC-001-T03`, `SEC-001-T04`, `SEC-001-T05`
- `SEC-003` (accepted): TBD - see source audit Obligations: `SEC-003-T01`, `SEC-003-T02`, `SEC-003-T03`, `SEC-003-T04`, `SEC-003-T05`
- `UI-008` (not-started): TBD - see source audit Obligations: `UI-008-T01`, `UI-008-T02`, `UI-008-T03`, `UI-008-T04`, `UI-008-T05`
- `TOOL-010` (accepted): request_permissions mid-turn tool Obligations: `TOOL-010-T01`..`T05`

### REQ-022 - 9router-style built-in multi-account routing

- `ROUTE-001` (not-started): TBD - see source audit Obligations: `ROUTE-001-T01`, `ROUTE-001-T02`, `ROUTE-001-T03`, `ROUTE-001-T04`, `ROUTE-001-T05`
- `ROUTE-002` (not-started): TBD - see source audit Obligations: `ROUTE-002-T01`, `ROUTE-002-T02`, `ROUTE-002-T03`, `ROUTE-002-T04`, `ROUTE-002-T05`
- `ROUTE-005` (not-started): TBD - see source audit Obligations: `ROUTE-005-T01`, `ROUTE-005-T02`, `ROUTE-005-T03`, `ROUTE-005-T04`, `ROUTE-005-T05`
- `ROUTE-011` (not-started): TBD - see source audit Obligations: `ROUTE-011-T01`, `ROUTE-011-T02`, `ROUTE-011-T03`, `ROUTE-011-T04`, `ROUTE-011-T05`

### REQ-023 - Timestamps on each response

- `SESS-002` (accepted): TBD - see source audit Obligations: `SESS-002-T01`, `SESS-002-T02`, `SESS-002-T03`, `SESS-002-T04`, `SESS-002-T05`
- `UI-003` (not-started): TBD - see source audit Obligations: `UI-003-T01`, `UI-003-T02`, `UI-003-T03`, `UI-003-T04`, `UI-003-T05`

### REQ-024 - Lightweight delegation on basic PC or VPS

- `AGENT-015` (accepted): TBD - see source audit Obligations: `AGENT-015-T01`, `AGENT-015-T02`, `AGENT-015-T03`, `AGENT-015-T04`, `AGENT-015-T05`
- `OPS-001` (not-started): TBD - see source audit Obligations: `OPS-001-T01`, `OPS-001-T02`, `OPS-001-T03`, `OPS-001-T04`, `OPS-001-T05`
- `OPS-007` (not-started): TBD - see source audit Obligations: `OPS-007-T01`, `OPS-007-T02`, `OPS-007-T03`, `OPS-007-T04`, `OPS-007-T05`

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
- `EXT-013` (in-progress): Skill safe extraction Obligations: `EXT-013-T01`..`T05`

### REQ-028 - System files readable where safe but strictly not editable by agent

- `SEC-006` (accepted): TBD - see source audit Obligations: `SEC-006-T01`, `SEC-006-T02`, `SEC-006-T03`, `SEC-006-T04`, `SEC-006-T05`

### REQ-029 - Trusted user toggle for destructive-command protection

- `SEC-003` (accepted): TBD - see source audit Obligations: `SEC-003-T01`, `SEC-003-T02`, `SEC-003-T03`, `SEC-003-T04`, `SEC-003-T05`
- `SEC-006` (accepted): TBD - see source audit Obligations: `SEC-006-T01`, `SEC-006-T02`, `SEC-006-T03`, `SEC-006-T04`, `SEC-006-T05`
- `UI-011` (not-started): TBD - see source audit Obligations: `UI-011-T01`, `UI-011-T02`, `UI-011-T03`, `UI-011-T04`, `UI-011-T05`

### REQ-030 - Permission star cannot bypass mandatory controls

- `SEC-001` (accepted): TBD - see source audit Obligations: `SEC-001-T01`, `SEC-001-T02`, `SEC-001-T03`, `SEC-001-T04`, `SEC-001-T05`
- `SEC-003` (accepted): TBD - see source audit Obligations: `SEC-003-T01`, `SEC-003-T02`, `SEC-003-T03`, `SEC-003-T04`, `SEC-003-T05`
- `SEC-017` (accepted): TBD - see source audit Obligations: `SEC-017-T01`, `SEC-017-T02`, `SEC-017-T03`, `SEC-017-T04`, `SEC-017-T05`

### REQ-031 - Other projects and user files require explicit approval

- `SEC-007` (accepted): TBD - see source audit Obligations: `SEC-007-T01`, `SEC-007-T02`, `SEC-007-T03`, `SEC-007-T04`, `SEC-007-T05`

### REQ-032 - Many features configurable and no hidden cost when off

- `OPS-001` (not-started): TBD - see source audit Obligations: `OPS-001-T01`, `OPS-001-T02`, `OPS-001-T03`, `OPS-001-T04`, `OPS-001-T05`
- `BASE-006` (accepted): TBD - see source audit Obligations: `BASE-006-T01`, `BASE-006-T02`, `BASE-006-T03`, `BASE-006-T04`, `BASE-006-T05`
- `UI-018` (not-started): TUI keybindings help Obligations: `UI-018-T01`..`T05`
- `UI-011` (not-started): TBD - see source audit Obligations: `UI-011-T01`, `UI-011-T02`, `UI-011-T03`, `UI-011-T04`, `UI-011-T05`

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
- `REL-002` (not-started): TBD - see source audit Obligations: `REL-002-T01`, `REL-002-T02`, `REL-002-T03`, `REL-002-T04`, `REL-002-T05`
- `REL-003` (not-started): TBD - see source audit Obligations: `REL-003-T01`, `REL-003-T02`, `REL-003-T03`, `REL-003-T04`, `REL-003-T05`

### REQ-036 - Full subagent lifecycle control

### REQ-037 - Lean harness mining adoption

- `TOOL-006` (accepted): Typed tool contract via buildTool factory Obligations: `TOOL-006-T01`..`T05`
- `TOOL-009` (accepted): ToolSearch discovery for lean context Obligations: `TOOL-009-T01`..`T05`
- `TOOL-015` (in-progress): MCP tool policy gate plus elicitation Obligations: `TOOL-015-T01`..`T05`
- `OPS-010` (not-started): Doctor diagnostics command Obligations: `OPS-010-T01`..`T05`

### REQ-038 - Provider-boundary observability

- `PROV-013` (accepted): Provider-boundary tap with redaction Obligations: `PROV-013-T01`..`T05`
- `PROV-014` (in-progress): Debug export bundle plus offline renderer Obligations: `PROV-014-T01`..`T05`

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
| `AUTO-003` | `REQ-002` | in-progress | TBD - see source audit |
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
| `EXT-003` | - | in-progress | Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json |
| `EXT-004` | - | in-progress | Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json |
| `EXT-005` | `REQ-005` | in-progress | TBD - see source audit |
| `EXT-006` | - | in-progress | Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json |
| `EXT-007` | - | in-progress | Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json |
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
| `INT-004` | - | in-progress | Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json |
| `INT-005` | - | in-progress | Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json |
| `INT-006` | - | in-progress | Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json |
| `INT-007` | - | in-progress | Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json |
| `INT-008` | - | not-started | Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json |
| `INT-009` | - | not-started | Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json |
| `INT-010` | - | not-started | Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json |

### OPS (10 stories)

| Story | Requirements | Status | User story |
|---|---|---|---|
| `OPS-001` | `REQ-001`, `REQ-024`, `REQ-032` | not-started | TBD - see source audit |
| `OPS-002` | - | not-started | Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json |
| `OPS-003` | - | not-started | Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json |
| `OPS-004` | - | not-started | Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json |
| `OPS-005` | - | not-started | Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json |
| `OPS-006` | - | not-started | Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json |
| `OPS-007` | `REQ-001`, `REQ-024` | not-started | TBD - see source audit |
| `OPS-008` | - | not-started | Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json |
| `OPS-009` | - | not-started | Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json |

### PROV (14 stories)

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

### REL (4 stories)

| Story | Requirements | Status | User story |
|---|---|---|---|
| `REL-001` | `REQ-003` | not-started | TBD - see source audit |
| `REL-002` | `REQ-004`, `REQ-035` | not-started | TBD - see source audit |
| `REL-003` | `REQ-035` | not-started | TBD - see source audit |

### ROUTE (12 stories)

| Story | Requirements | Status | User story |
|---|---|---|---|
| `ROUTE-001` | `REQ-022` | not-started | TBD - see source audit |
| `ROUTE-002` | `REQ-022` | not-started | TBD - see source audit |
| `ROUTE-003` | - | not-started | Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json |
| `ROUTE-004` | - | not-started | Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json |
| `ROUTE-005` | `REQ-022` | not-started | TBD - see source audit |
| `ROUTE-006` | - | not-started | Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json |
| `ROUTE-007` | - | not-started | Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json |
| `ROUTE-008` | `REQ-016` | not-started | TBD - see source audit |
| `ROUTE-009` | - | not-started | Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json |
| `ROUTE-010` | - | not-started | Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json |
| `ROUTE-011` | `REQ-022` | not-started | TBD - see source audit |

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
| `SHARE-001` | `REQ-007` | not-started | TBD - see source audit |
| `SHARE-002` | - | not-started | Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json |
| `SHARE-003` | - | not-started | Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json |
| `SHARE-004` | `REQ-007` | not-started | TBD - see source audit |
| `SHARE-005` | `REQ-007` | not-started | TBD - see source audit |

### TOOL (15 stories)

| Story | Requirements | Status | User story |
|---|---|---|---|
| `TOOL-001` | - | accepted | Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json |
| `TOOL-002` | - | accepted | Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json |
| `TOOL-003` | - | accepted | Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json |
| `TOOL-004` | - | accepted | Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json |
| `TOOL-005` | - | accepted | Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json |
| `TOOL-008` | - | accepted | Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json |
| `TOOL-011` | - | accepted | Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json |
| `TOOL-012` | - | accepted | Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json |
| `TOOL-013` | - | accepted | Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json |
| `TOOL-014` | - | accepted | Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json |

### UI (18 stories)

| Story | Requirements | Status | User story |
|---|---|---|---|
| `UI-001` | - | not-started | Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json |
| `UI-002` | - | not-started | Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json |
| `UI-003` | `REQ-023` | not-started | TBD - see source audit |
| `UI-004` | `REQ-008` | not-started | TBD - see source audit |
| `UI-005` | `REQ-013` | not-started | TBD - see source audit |
| `UI-006` | `REQ-016` | not-started | TBD - see source audit |
| `UI-007` | `REQ-018` | not-started | TBD - see source audit |
| `UI-008` | `REQ-021` | not-started | TBD - see source audit |
| `UI-009` | `REQ-019` | not-started | TBD - see source audit |
| `UI-010` | `REQ-017` | not-started | TBD - see source audit |
| `UI-011` | `REQ-029`, `REQ-032` | not-started | TBD - see source audit |
| `UI-012` | `REQ-005` | not-started | TBD - see source audit |
| `UI-013` | - | not-started | Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json |

### WEB (5 stories)

| Story | Requirements | Status | User story |
|---|---|---|---|
| `WEB-001` | - | not-started | Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json |
| `WEB-002` | - | not-started | Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json |
| `WEB-003` | - | not-started | Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json |
| `WEB-004` | - | not-started | Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json |
| `WEB-005` | - | not-started | Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json |

## Stories without a direct requirement link (96)

| `TOOL-006` | `REQ-037` | accepted | Typed tool contract via buildTool factory - schema plus permission plus exec plus render in one definition |
| `TOOL-007` | `REQ-017` | accepted | Skill-gated approvals - skill invocation passes the same policy gate as tools |
| `TOOL-009` | `REQ-037` | accepted | ToolSearch discovery - just-in-time tool listing to keep lean context |
| `TOOL-010` | `REQ-021`, `REQ-026` | accepted | request_permissions mid-turn tool - agent asks for a named capability with scope, rate-limited |
| `TOOL-015` | `REQ-037` | in-progress | MCP tool own policy gate plus elicitation path with timeout and cancel |
| `SEC-018` | `REQ-025` | accepted | Dangerous shell-pattern denylist stripped at auto-entry - cross-platform code-exec block plus matcher tests |
| `SEC-019` | `REQ-025`, `REQ-030` | accepted | Execpolicy prefix rules - declarative path and command allowlist, longest-match evaluated before tools |
| `SEC-020` | `REQ-020` | accepted | Bounded hook bus - max 100 pending with shift-drop, always-emit allowlist, SSRF guard on HTTP hooks |
| `AUTO-007` | `REQ-002`, `REQ-012` | in-progress | Turn submission state machine - submit to running to interrupted or complete as typed transitions with cancel cleanup |
| `DB-017` | `REQ-006`, `REQ-033` | accepted | Dual rollout record - append-only JSONL is truth, sqlite holds queryable snapshot, recorder owns order |
| `DB-018` | `REQ-033` | accepted | Content-addressed transcript dedupe - repeated prompts and tool schemas stored by canonical hash, turns reference ids |
| `PROV-013` | `REQ-038` | accepted | Provider-boundary tap with redaction - uid-keyed structured records at request, response, stream-final, error seam |
| `PROV-014` | `REQ-038` | in-progress | Debug export bundle - one redacted JSONL with session meta, deduped defs, turns, timings, errors; offline renderer |
| `OPS-010` | `REQ-037` | not-started | Doctor diagnostics command - checks auth, connectivity, tools, MCP; reports failures without log spelunking |
| `EXT-013` | `REQ-017`, `REQ-027` | in-progress | Skill safe extraction - bundled files materialize once, owner-only 0600 O_EXCL O_NOFOLLOW, traversal rejected |
| `SESS-018` | `REQ-008` | accepted | Fork boundary is typed - branch copy only on fork, cache cleared at boundary |
| `SESS-019` | `REQ-008` | in-progress | Revert is free pointer move distinct from truncating rollback with separate API names |
| `SESS-020` | `REQ-006` | in-progress | Auto-compact thresholds plus warning states plus 3-strike circuit breaker with env kill-switches |
| `ROUTE-012` | `REQ-018` | in-progress | Cheap-model chores routing - quota, topic, title, summarize go to cheapest capable model, fail-open |
| `AGENT-031` | `REQ-013` | accepted | Plan mode plus structured review child - plan permission mode with Enter and Exit tools, review returns machine-checkable findings |
| `REL-004` | `REQ-016` | not-started | Per-turn cost counters - input, output, cache-read, cache-create, durations, lines changed, web-search count, unknown-cost flag |
| `UI-014` | `REQ-016`, `REQ-018` | not-started | TUI composer - multiline editor, Enter sends, Shift+Enter newline, Ctrl+J fallback, send button, draft survives interrupt, queue-while-busy |
| `UI-015` | `REQ-016` | not-started | TUI status bar - clickable model item opens provider-aware switcher, clickable context item opens context detail view; keyboard fallback |
| `UI-016` | `REQ-016` | not-started | TUI context detail view - per-source token breakdown with cache-read vs fresh flags, largest blocks first, truncation markers |
| `UI-017` | `REQ-006`, `REQ-016` | not-started | TUI /memory viewer - list loaded memory files with path bytes tokens cached flag, unload with prefix-cache cost warning, reload path |
| `UI-018` | `REQ-019` | not-started | TUI keybindings help - footer hints plus /keybindings parity, configurable composer submit keymap |
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
- `EXT-003` (in-progress): Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json
- `EXT-004` (in-progress): Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json
- `EXT-006` (in-progress): Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json
- `EXT-007` (in-progress): Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json
- `EXT-010` (in-progress): Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json
- `EXT-011` (in-progress): Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json
- `INT-001` (in-progress): Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json
- `INT-002` (in-progress): Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json
- `INT-003` (in-progress): Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json
- `INT-004` (in-progress): Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json
- `INT-005` (in-progress): Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json
- `INT-006` (in-progress): Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json
- `INT-007` (in-progress): Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json
- `INT-008` (not-started): Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json
- `INT-009` (not-started): Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json
- `INT-010` (not-started): Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json
- `OPS-002` (not-started): Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json
- `OPS-003` (not-started): Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json
- `OPS-004` (not-started): Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json
- `OPS-005` (not-started): Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json
- `OPS-006` (not-started): Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json
- `OPS-008` (not-started): Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json
- `OPS-009` (not-started): Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json
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
- `ROUTE-003` (not-started): Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json
- `ROUTE-004` (not-started): Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json
- `ROUTE-006` (not-started): Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json
- `ROUTE-007` (not-started): Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json
- `ROUTE-009` (not-started): Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json
- `ROUTE-010` (not-started): Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json
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
- `SHARE-002` (not-started): Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json
- `SHARE-003` (not-started): Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json
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
- `TOOL-015` (in-progress): MCP tool policy gate plus elicitation Obligations: `TOOL-015-T01`..`T05`
- `UI-001` (not-started): Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json
- `UI-002` (not-started): Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json
- `UI-013` (not-started): Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json
- `WEB-001` (not-started): Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json
- `WEB-002` (not-started): Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json
- `WEB-003` (not-started): Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json
- `WEB-004` (not-started): Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json
- `WEB-005` (not-started): Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json
