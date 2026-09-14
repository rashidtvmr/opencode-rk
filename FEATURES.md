# FEATURES - navigable feature index

Generated from `ralph.json` (canonical extended plan) and `requirements/user-requirements.json`.
Every story is mandatory to the declared full release even when its feature is off by default at runtime (PLAN.md section 1).
`prd.json` is the flat conventional Ralph export of the same data. Dependency eligibility is decided by `ralph.json` plus the milestone ranks in PLAN.md section 4; see `tools/plan_model.py`.

Stories: 178. Requirements: 35. Test obligations: 890.

## Requirements to stories

| Requirement | Capability | Stories |
|---|---|---|
| `REQ-001` | Rust plus Tokio and maximum practical resource savings | `BASE-003`, `OPS-001`, `OPS-007` |
| `REQ-002` | Plan file Ralph JSON independent slices and autonomous loop | `AUTO-001`, `AUTO-002`, `AUTO-003`, `AUTO-004`, `AUTO-005`, `AUTO-006` |
| `REQ-003` | Every OpenCode V2 feature accounted for | `DISC-001`, `DISC-010`, `REL-001` |
| `REQ-004` | Strict TDD and independent verification | `AUTO-002`, `AUTO-005`, `REL-002` |
| `REQ-005` | OpenCode V2 plugins including UI behavior | `EXT-005`, `EXT-009`, `EXT-012`, `UI-012` |
| `REQ-006` | Session management and persistent history | `SESS-001`, `DB-003`, `SESS-017` |
| `REQ-007` | Session sharing | `SHARE-001`, `SHARE-004`, `SHARE-005` |
| `REQ-008` | Session forking | `SESS-011`, `UI-004` |
| `REQ-009` | Multiple delegation types and custom agents | `AGENT-002`, `AGENT-003`, `AGENT-004`, `AGENT-010` |
| `REQ-010` | Cross-provider subagent delegation | `AGENT-005`, `PROV-011` |
| `REQ-011` | Own searchable models.dev-backed API | `CAT-001`, `CAT-002`, `CAT-007` |
| `REQ-012` | Foreground and background subagents | `AGENT-003`, `AGENT-004` |
| `REQ-013` | Navigate resume and steer subagents | `AGENT-007`, `AGENT-008`, `UI-005` |
| `REQ-014` | Subagent context and messages in DB | `AGENT-011`, `DB-003` |
| `REQ-015` | Singleton supports many application clients | `BASE-004`, `BASE-005` |
| `REQ-016` | Status panel | `UI-006`, `ROUTE-008` |
| `REQ-017` | Skills plugins and custom slash commands | `EXT-001`, `EXT-002`, `UI-010` |
| `REQ-018` | Independent main and child effort levels | `CAT-004`, `AGENT-006`, `UI-007` |
| `REQ-019` | Themes and common settings | `UI-009`, `BASE-006` |
| `REQ-020` | Pre-tool and post-tool hooks | `SEC-010`, `SEC-011`, `EXT-008` |
| `REQ-021` | Permissions and mandatory human-in-the-loop | `SEC-001`, `SEC-003`, `UI-008` |
| `REQ-022` | 9router-style built-in multi-account routing | `ROUTE-001`, `ROUTE-002`, `ROUTE-005`, `ROUTE-011` |
| `REQ-023` | Timestamps on each response | `SESS-002`, `UI-003` |
| `REQ-024` | Lightweight delegation on basic PC or VPS | `AGENT-015`, `OPS-001`, `OPS-007` |
| `REQ-025` | Deterministic destructive-command and SQL controls | `SEC-002`, `SEC-012`, `SEC-016` |
| `REQ-026` | Manual-only destructive instructions rather than agent execution | `SEC-013` |
| `REQ-027` | OS-enforced .env and sensitive-file restrictions | `SEC-004`, `SEC-005`, `SEC-014` |
| `REQ-028` | System files readable where safe but strictly not editable by agent | `SEC-006` |
| `REQ-029` | Trusted user toggle for destructive-command protection | `SEC-003`, `SEC-006`, `UI-011` |
| `REQ-030` | Permission star cannot bypass mandatory controls | `SEC-001`, `SEC-003`, `SEC-017` |
| `REQ-031` | Other projects and user files require explicit approval | `SEC-007` |
| `REQ-032` | Many features configurable and no hidden cost when off | `OPS-001`, `BASE-006`, `UI-011` |
| `REQ-033` | Embedded scalable storage without Postgres or Mongo install | `DB-001`, `DB-006`, `DB-008`, `DB-009` |
| `REQ-034` | Safely import large existing OpenCode data | `DB-012`, `DB-016` |
| `REQ-035` | Preserve safety and resource correctness rather than blindly translate files | `DISC-008`, `SEC-017`, `REL-002`, `REL-003` |

## Requirement detail

### REQ-001 - Rust plus Tokio and maximum practical resource savings

- `BASE-003` (not-started): TBD - see source audit Obligations: `BASE-003-T01`, `BASE-003-T02`, `BASE-003-T03`, `BASE-003-T04`, `BASE-003-T05`
- `OPS-001` (not-started): TBD - see source audit Obligations: `OPS-001-T01`, `OPS-001-T02`, `OPS-001-T03`, `OPS-001-T04`, `OPS-001-T05`
- `OPS-007` (not-started): TBD - see source audit Obligations: `OPS-007-T01`, `OPS-007-T02`, `OPS-007-T03`, `OPS-007-T04`, `OPS-007-T05`

### REQ-002 - Plan file Ralph JSON independent slices and autonomous loop

- `AUTO-001` (not-started): TBD - see source audit Obligations: `AUTO-001-T01`, `AUTO-001-T02`, `AUTO-001-T03`, `AUTO-001-T04`, `AUTO-001-T05`
- `AUTO-002` (not-started): TBD - see source audit Obligations: `AUTO-002-T01`, `AUTO-002-T02`, `AUTO-002-T03`, `AUTO-002-T04`, `AUTO-002-T05`
- `AUTO-003` (not-started): TBD - see source audit Obligations: `AUTO-003-T01`, `AUTO-003-T02`, `AUTO-003-T03`, `AUTO-003-T04`, `AUTO-003-T05`
- `AUTO-004` (not-started): TBD - see source audit Obligations: `AUTO-004-T01`, `AUTO-004-T02`, `AUTO-004-T03`, `AUTO-004-T04`, `AUTO-004-T05`
- `AUTO-005` (not-started): TBD - see source audit Obligations: `AUTO-005-T01`, `AUTO-005-T02`, `AUTO-005-T03`, `AUTO-005-T04`, `AUTO-005-T05`
- `AUTO-006` (not-started): TBD - see source audit Obligations: `AUTO-006-T01`, `AUTO-006-T02`, `AUTO-006-T03`, `AUTO-006-T04`, `AUTO-006-T05`

### REQ-003 - Every OpenCode V2 feature accounted for

- `DISC-001` (not-started): TBD - see source audit Obligations: `DISC-001-T01`, `DISC-001-T02`, `DISC-001-T03`, `DISC-001-T04`, `DISC-001-T05`
- `DISC-010` (not-started): TBD - see source audit Obligations: `DISC-010-T01`, `DISC-010-T02`, `DISC-010-T03`, `DISC-010-T04`, `DISC-010-T05`
- `REL-001` (not-started): TBD - see source audit Obligations: `REL-001-T01`, `REL-001-T02`, `REL-001-T03`, `REL-001-T04`, `REL-001-T05`

### REQ-004 - Strict TDD and independent verification

- `AUTO-002` (not-started): TBD - see source audit Obligations: `AUTO-002-T01`, `AUTO-002-T02`, `AUTO-002-T03`, `AUTO-002-T04`, `AUTO-002-T05`
- `AUTO-005` (not-started): TBD - see source audit Obligations: `AUTO-005-T01`, `AUTO-005-T02`, `AUTO-005-T03`, `AUTO-005-T04`, `AUTO-005-T05`
- `REL-002` (not-started): TBD - see source audit Obligations: `REL-002-T01`, `REL-002-T02`, `REL-002-T03`, `REL-002-T04`, `REL-002-T05`

### REQ-005 - OpenCode V2 plugins including UI behavior

- `EXT-005` (not-started): TBD - see source audit Obligations: `EXT-005-T01`, `EXT-005-T02`, `EXT-005-T03`, `EXT-005-T04`, `EXT-005-T05`
- `EXT-009` (not-started): TBD - see source audit Obligations: `EXT-009-T01`, `EXT-009-T02`, `EXT-009-T03`, `EXT-009-T04`, `EXT-009-T05`
- `EXT-012` (not-started): TBD - see source audit Obligations: `EXT-012-T01`, `EXT-012-T02`, `EXT-012-T03`, `EXT-012-T04`, `EXT-012-T05`
- `UI-012` (not-started): TBD - see source audit Obligations: `UI-012-T01`, `UI-012-T02`, `UI-012-T03`, `UI-012-T04`, `UI-012-T05`

### REQ-006 - Session management and persistent history

- `SESS-001` (not-started): TBD - see source audit Obligations: `SESS-001-T01`, `SESS-001-T02`, `SESS-001-T03`, `SESS-001-T04`, `SESS-001-T05`
- `DB-003` (not-started): TBD - see source audit Obligations: `DB-003-T01`, `DB-003-T02`, `DB-003-T03`, `DB-003-T04`, `DB-003-T05`
- `SESS-017` (not-started): TBD - see source audit Obligations: `SESS-017-T01`, `SESS-017-T02`, `SESS-017-T03`, `SESS-017-T04`, `SESS-017-T05`

### REQ-007 - Session sharing

- `SHARE-001` (not-started): TBD - see source audit Obligations: `SHARE-001-T01`, `SHARE-001-T02`, `SHARE-001-T03`, `SHARE-001-T04`, `SHARE-001-T05`
- `SHARE-004` (not-started): TBD - see source audit Obligations: `SHARE-004-T01`, `SHARE-004-T02`, `SHARE-004-T03`, `SHARE-004-T04`, `SHARE-004-T05`
- `SHARE-005` (not-started): TBD - see source audit Obligations: `SHARE-005-T01`, `SHARE-005-T02`, `SHARE-005-T03`, `SHARE-005-T04`, `SHARE-005-T05`

### REQ-008 - Session forking

- `SESS-011` (not-started): TBD - see source audit Obligations: `SESS-011-T01`, `SESS-011-T02`, `SESS-011-T03`, `SESS-011-T04`, `SESS-011-T05`
- `UI-004` (not-started): TBD - see source audit Obligations: `UI-004-T01`, `UI-004-T02`, `UI-004-T03`, `UI-004-T04`, `UI-004-T05`

### REQ-009 - Multiple delegation types and custom agents

- `AGENT-002` (not-started): TBD - see source audit Obligations: `AGENT-002-T01`, `AGENT-002-T02`, `AGENT-002-T03`, `AGENT-002-T04`, `AGENT-002-T05`
- `AGENT-003` (not-started): TBD - see source audit Obligations: `AGENT-003-T01`, `AGENT-003-T02`, `AGENT-003-T03`, `AGENT-003-T04`, `AGENT-003-T05`
- `AGENT-004` (not-started): TBD - see source audit Obligations: `AGENT-004-T01`, `AGENT-004-T02`, `AGENT-004-T03`, `AGENT-004-T04`, `AGENT-004-T05`
- `AGENT-010` (not-started): TBD - see source audit Obligations: `AGENT-010-T01`, `AGENT-010-T02`, `AGENT-010-T03`, `AGENT-010-T04`, `AGENT-010-T05`

### REQ-010 - Cross-provider subagent delegation

- `AGENT-005` (not-started): TBD - see source audit Obligations: `AGENT-005-T01`, `AGENT-005-T02`, `AGENT-005-T03`, `AGENT-005-T04`, `AGENT-005-T05`
- `PROV-011` (not-started): TBD - see source audit Obligations: `PROV-011-T01`, `PROV-011-T02`, `PROV-011-T03`, `PROV-011-T04`, `PROV-011-T05`

### REQ-011 - Own searchable models.dev-backed API

- `CAT-001` (not-started): TBD - see source audit Obligations: `CAT-001-T01`, `CAT-001-T02`, `CAT-001-T03`, `CAT-001-T04`, `CAT-001-T05`
- `CAT-002` (not-started): TBD - see source audit Obligations: `CAT-002-T01`, `CAT-002-T02`, `CAT-002-T03`, `CAT-002-T04`, `CAT-002-T05`
- `CAT-007` (not-started): TBD - see source audit Obligations: `CAT-007-T01`, `CAT-007-T02`, `CAT-007-T03`, `CAT-007-T04`, `CAT-007-T05`

### REQ-012 - Foreground and background subagents

- `AGENT-003` (not-started): TBD - see source audit Obligations: `AGENT-003-T01`, `AGENT-003-T02`, `AGENT-003-T03`, `AGENT-003-T04`, `AGENT-003-T05`
- `AGENT-004` (not-started): TBD - see source audit Obligations: `AGENT-004-T01`, `AGENT-004-T02`, `AGENT-004-T03`, `AGENT-004-T04`, `AGENT-004-T05`

### REQ-013 - Navigate resume and steer subagents

- `AGENT-007` (not-started): TBD - see source audit Obligations: `AGENT-007-T01`, `AGENT-007-T02`, `AGENT-007-T03`, `AGENT-007-T04`, `AGENT-007-T05`
- `AGENT-008` (not-started): TBD - see source audit Obligations: `AGENT-008-T01`, `AGENT-008-T02`, `AGENT-008-T03`, `AGENT-008-T04`, `AGENT-008-T05`
- `UI-005` (not-started): TBD - see source audit Obligations: `UI-005-T01`, `UI-005-T02`, `UI-005-T03`, `UI-005-T04`, `UI-005-T05`

### REQ-014 - Subagent context and messages in DB

- `AGENT-011` (not-started): TBD - see source audit Obligations: `AGENT-011-T01`, `AGENT-011-T02`, `AGENT-011-T03`, `AGENT-011-T04`, `AGENT-011-T05`
- `DB-003` (not-started): TBD - see source audit Obligations: `DB-003-T01`, `DB-003-T02`, `DB-003-T03`, `DB-003-T04`, `DB-003-T05`

### REQ-015 - Singleton supports many application clients

- `BASE-004` (not-started): TBD - see source audit Obligations: `BASE-004-T01`, `BASE-004-T02`, `BASE-004-T03`, `BASE-004-T04`, `BASE-004-T05`
- `BASE-005` (not-started): TBD - see source audit Obligations: `BASE-005-T01`, `BASE-005-T02`, `BASE-005-T03`, `BASE-005-T04`, `BASE-005-T05`

### REQ-016 - Status panel

- `UI-006` (not-started): TBD - see source audit Obligations: `UI-006-T01`, `UI-006-T02`, `UI-006-T03`, `UI-006-T04`, `UI-006-T05`
- `ROUTE-008` (not-started): TBD - see source audit Obligations: `ROUTE-008-T01`, `ROUTE-008-T02`, `ROUTE-008-T03`, `ROUTE-008-T04`, `ROUTE-008-T05`

### REQ-017 - Skills plugins and custom slash commands

- `EXT-001` (not-started): TBD - see source audit Obligations: `EXT-001-T01`, `EXT-001-T02`, `EXT-001-T03`, `EXT-001-T04`, `EXT-001-T05`
- `EXT-002` (not-started): TBD - see source audit Obligations: `EXT-002-T01`, `EXT-002-T02`, `EXT-002-T03`, `EXT-002-T04`, `EXT-002-T05`
- `UI-010` (not-started): TBD - see source audit Obligations: `UI-010-T01`, `UI-010-T02`, `UI-010-T03`, `UI-010-T04`, `UI-010-T05`

### REQ-018 - Independent main and child effort levels

- `CAT-004` (not-started): TBD - see source audit Obligations: `CAT-004-T01`, `CAT-004-T02`, `CAT-004-T03`, `CAT-004-T04`, `CAT-004-T05`
- `AGENT-006` (not-started): TBD - see source audit Obligations: `AGENT-006-T01`, `AGENT-006-T02`, `AGENT-006-T03`, `AGENT-006-T04`, `AGENT-006-T05`
- `UI-007` (not-started): TBD - see source audit Obligations: `UI-007-T01`, `UI-007-T02`, `UI-007-T03`, `UI-007-T04`, `UI-007-T05`

### REQ-019 - Themes and common settings

- `UI-009` (not-started): TBD - see source audit Obligations: `UI-009-T01`, `UI-009-T02`, `UI-009-T03`, `UI-009-T04`, `UI-009-T05`
- `BASE-006` (not-started): TBD - see source audit Obligations: `BASE-006-T01`, `BASE-006-T02`, `BASE-006-T03`, `BASE-006-T04`, `BASE-006-T05`

### REQ-020 - Pre-tool and post-tool hooks

- `SEC-010` (not-started): TBD - see source audit Obligations: `SEC-010-T01`, `SEC-010-T02`, `SEC-010-T03`, `SEC-010-T04`, `SEC-010-T05`
- `SEC-011` (not-started): TBD - see source audit Obligations: `SEC-011-T01`, `SEC-011-T02`, `SEC-011-T03`, `SEC-011-T04`, `SEC-011-T05`
- `EXT-008` (not-started): TBD - see source audit Obligations: `EXT-008-T01`, `EXT-008-T02`, `EXT-008-T03`, `EXT-008-T04`, `EXT-008-T05`

### REQ-021 - Permissions and mandatory human-in-the-loop

- `SEC-001` (not-started): TBD - see source audit Obligations: `SEC-001-T01`, `SEC-001-T02`, `SEC-001-T03`, `SEC-001-T04`, `SEC-001-T05`
- `SEC-003` (not-started): TBD - see source audit Obligations: `SEC-003-T01`, `SEC-003-T02`, `SEC-003-T03`, `SEC-003-T04`, `SEC-003-T05`
- `UI-008` (not-started): TBD - see source audit Obligations: `UI-008-T01`, `UI-008-T02`, `UI-008-T03`, `UI-008-T04`, `UI-008-T05`

### REQ-022 - 9router-style built-in multi-account routing

- `ROUTE-001` (not-started): TBD - see source audit Obligations: `ROUTE-001-T01`, `ROUTE-001-T02`, `ROUTE-001-T03`, `ROUTE-001-T04`, `ROUTE-001-T05`
- `ROUTE-002` (not-started): TBD - see source audit Obligations: `ROUTE-002-T01`, `ROUTE-002-T02`, `ROUTE-002-T03`, `ROUTE-002-T04`, `ROUTE-002-T05`
- `ROUTE-005` (not-started): TBD - see source audit Obligations: `ROUTE-005-T01`, `ROUTE-005-T02`, `ROUTE-005-T03`, `ROUTE-005-T04`, `ROUTE-005-T05`
- `ROUTE-011` (not-started): TBD - see source audit Obligations: `ROUTE-011-T01`, `ROUTE-011-T02`, `ROUTE-011-T03`, `ROUTE-011-T04`, `ROUTE-011-T05`

### REQ-023 - Timestamps on each response

- `SESS-002` (not-started): TBD - see source audit Obligations: `SESS-002-T01`, `SESS-002-T02`, `SESS-002-T03`, `SESS-002-T04`, `SESS-002-T05`
- `UI-003` (not-started): TBD - see source audit Obligations: `UI-003-T01`, `UI-003-T02`, `UI-003-T03`, `UI-003-T04`, `UI-003-T05`

### REQ-024 - Lightweight delegation on basic PC or VPS

- `AGENT-015` (not-started): TBD - see source audit Obligations: `AGENT-015-T01`, `AGENT-015-T02`, `AGENT-015-T03`, `AGENT-015-T04`, `AGENT-015-T05`
- `OPS-001` (not-started): TBD - see source audit Obligations: `OPS-001-T01`, `OPS-001-T02`, `OPS-001-T03`, `OPS-001-T04`, `OPS-001-T05`
- `OPS-007` (not-started): TBD - see source audit Obligations: `OPS-007-T01`, `OPS-007-T02`, `OPS-007-T03`, `OPS-007-T04`, `OPS-007-T05`

### REQ-025 - Deterministic destructive-command and SQL controls

- `SEC-002` (not-started): TBD - see source audit Obligations: `SEC-002-T01`, `SEC-002-T02`, `SEC-002-T03`, `SEC-002-T04`, `SEC-002-T05`
- `SEC-012` (not-started): TBD - see source audit Obligations: `SEC-012-T01`, `SEC-012-T02`, `SEC-012-T03`, `SEC-012-T04`, `SEC-012-T05`
- `SEC-016` (not-started): TBD - see source audit Obligations: `SEC-016-T01`, `SEC-016-T02`, `SEC-016-T03`, `SEC-016-T04`, `SEC-016-T05`

### REQ-026 - Manual-only destructive instructions rather than agent execution

- `SEC-013` (not-started): TBD - see source audit Obligations: `SEC-013-T01`, `SEC-013-T02`, `SEC-013-T03`, `SEC-013-T04`, `SEC-013-T05`

### REQ-027 - OS-enforced .env and sensitive-file restrictions

- `SEC-004` (not-started): TBD - see source audit Obligations: `SEC-004-T01`, `SEC-004-T02`, `SEC-004-T03`, `SEC-004-T04`, `SEC-004-T05`
- `SEC-005` (not-started): TBD - see source audit Obligations: `SEC-005-T01`, `SEC-005-T02`, `SEC-005-T03`, `SEC-005-T04`, `SEC-005-T05`
- `SEC-014` (not-started): TBD - see source audit Obligations: `SEC-014-T01`, `SEC-014-T02`, `SEC-014-T03`, `SEC-014-T04`, `SEC-014-T05`

### REQ-028 - System files readable where safe but strictly not editable by agent

- `SEC-006` (not-started): TBD - see source audit Obligations: `SEC-006-T01`, `SEC-006-T02`, `SEC-006-T03`, `SEC-006-T04`, `SEC-006-T05`

### REQ-029 - Trusted user toggle for destructive-command protection

- `SEC-003` (not-started): TBD - see source audit Obligations: `SEC-003-T01`, `SEC-003-T02`, `SEC-003-T03`, `SEC-003-T04`, `SEC-003-T05`
- `SEC-006` (not-started): TBD - see source audit Obligations: `SEC-006-T01`, `SEC-006-T02`, `SEC-006-T03`, `SEC-006-T04`, `SEC-006-T05`
- `UI-011` (not-started): TBD - see source audit Obligations: `UI-011-T01`, `UI-011-T02`, `UI-011-T03`, `UI-011-T04`, `UI-011-T05`

### REQ-030 - Permission star cannot bypass mandatory controls

- `SEC-001` (not-started): TBD - see source audit Obligations: `SEC-001-T01`, `SEC-001-T02`, `SEC-001-T03`, `SEC-001-T04`, `SEC-001-T05`
- `SEC-003` (not-started): TBD - see source audit Obligations: `SEC-003-T01`, `SEC-003-T02`, `SEC-003-T03`, `SEC-003-T04`, `SEC-003-T05`
- `SEC-017` (not-started): TBD - see source audit Obligations: `SEC-017-T01`, `SEC-017-T02`, `SEC-017-T03`, `SEC-017-T04`, `SEC-017-T05`

### REQ-031 - Other projects and user files require explicit approval

- `SEC-007` (not-started): TBD - see source audit Obligations: `SEC-007-T01`, `SEC-007-T02`, `SEC-007-T03`, `SEC-007-T04`, `SEC-007-T05`

### REQ-032 - Many features configurable and no hidden cost when off

- `OPS-001` (not-started): TBD - see source audit Obligations: `OPS-001-T01`, `OPS-001-T02`, `OPS-001-T03`, `OPS-001-T04`, `OPS-001-T05`
- `BASE-006` (not-started): TBD - see source audit Obligations: `BASE-006-T01`, `BASE-006-T02`, `BASE-006-T03`, `BASE-006-T04`, `BASE-006-T05`
- `UI-011` (not-started): TBD - see source audit Obligations: `UI-011-T01`, `UI-011-T02`, `UI-011-T03`, `UI-011-T04`, `UI-011-T05`

### REQ-033 - Embedded scalable storage without Postgres or Mongo install

- `DB-001` (not-started): TBD - see source audit Obligations: `DB-001-T01`, `DB-001-T02`, `DB-001-T03`, `DB-001-T04`, `DB-001-T05`
- `DB-006` (not-started): TBD - see source audit Obligations: `DB-006-T01`, `DB-006-T02`, `DB-006-T03`, `DB-006-T04`, `DB-006-T05`
- `DB-008` (not-started): TBD - see source audit Obligations: `DB-008-T01`, `DB-008-T02`, `DB-008-T03`, `DB-008-T04`, `DB-008-T05`
- `DB-009` (not-started): TBD - see source audit Obligations: `DB-009-T01`, `DB-009-T02`, `DB-009-T03`, `DB-009-T04`, `DB-009-T05`

### REQ-034 - Safely import large existing OpenCode data

- `DB-012` (not-started): TBD - see source audit Obligations: `DB-012-T01`, `DB-012-T02`, `DB-012-T03`, `DB-012-T04`, `DB-012-T05`
- `DB-016` (not-started): TBD - see source audit Obligations: `DB-016-T01`, `DB-016-T02`, `DB-016-T03`, `DB-016-T04`, `DB-016-T05`

### REQ-035 - Preserve safety and resource correctness rather than blindly translate files

- `DISC-008` (not-started): TBD - see source audit Obligations: `DISC-008-T01`, `DISC-008-T02`, `DISC-008-T03`, `DISC-008-T04`, `DISC-008-T05`
- `SEC-017` (not-started): TBD - see source audit Obligations: `SEC-017-T01`, `SEC-017-T02`, `SEC-017-T03`, `SEC-017-T04`, `SEC-017-T05`
- `REL-002` (not-started): TBD - see source audit Obligations: `REL-002-T01`, `REL-002-T02`, `REL-002-T03`, `REL-002-T04`, `REL-002-T05`
- `REL-003` (not-started): TBD - see source audit Obligations: `REL-003-T01`, `REL-003-T02`, `REL-003-T03`, `REL-003-T04`, `REL-003-T05`

## Story families (prefix index)

### AGENT (15 stories)

| Story | Requirements | Status | User story |
|---|---|---|---|
| `AGENT-001` | - | not-started | Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json |
| `AGENT-002` | `REQ-009` | not-started | TBD - see source audit |
| `AGENT-003` | `REQ-009`, `REQ-012` | not-started | TBD - see source audit |
| `AGENT-004` | `REQ-009`, `REQ-012` | not-started | TBD - see source audit |
| `AGENT-005` | `REQ-010` | not-started | TBD - see source audit |
| `AGENT-006` | `REQ-018` | not-started | TBD - see source audit |
| `AGENT-007` | `REQ-013` | not-started | TBD - see source audit |
| `AGENT-008` | `REQ-013` | not-started | TBD - see source audit |
| `AGENT-009` | - | not-started | Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json |
| `AGENT-010` | `REQ-009` | not-started | TBD - see source audit |
| `AGENT-011` | `REQ-014` | not-started | TBD - see source audit |
| `AGENT-012` | - | not-started | Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json |
| `AGENT-013` | - | not-started | Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json |
| `AGENT-014` | - | not-started | Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json |
| `AGENT-015` | `REQ-024` | not-started | TBD - see source audit |

### AUTO (6 stories)

| Story | Requirements | Status | User story |
|---|---|---|---|
| `AUTO-001` | `REQ-002` | not-started | TBD - see source audit |
| `AUTO-002` | `REQ-002`, `REQ-004` | not-started | TBD - see source audit |
| `AUTO-003` | `REQ-002` | not-started | TBD - see source audit |
| `AUTO-004` | `REQ-002` | not-started | TBD - see source audit |
| `AUTO-005` | `REQ-002`, `REQ-004` | not-started | TBD - see source audit |
| `AUTO-006` | `REQ-002` | not-started | TBD - see source audit |

### BASE (8 stories)

| Story | Requirements | Status | User story |
|---|---|---|---|
| `BASE-001` | - | not-started | Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json |
| `BASE-002` | - | not-started | Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json |
| `BASE-003` | `REQ-001` | not-started | TBD - see source audit |
| `BASE-004` | `REQ-015` | not-started | TBD - see source audit |
| `BASE-005` | `REQ-015` | not-started | TBD - see source audit |
| `BASE-006` | `REQ-019`, `REQ-032` | not-started | TBD - see source audit |
| `BASE-007` | - | not-started | Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json |
| `BASE-008` | - | not-started | Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json |

### CAT (7 stories)

| Story | Requirements | Status | User story |
|---|---|---|---|
| `CAT-001` | `REQ-011` | not-started | TBD - see source audit |
| `CAT-002` | `REQ-011` | not-started | TBD - see source audit |
| `CAT-003` | - | not-started | Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json |
| `CAT-004` | `REQ-018` | not-started | TBD - see source audit |
| `CAT-005` | - | not-started | Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json |
| `CAT-006` | - | not-started | Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json |
| `CAT-007` | `REQ-011` | not-started | TBD - see source audit |

### DB (16 stories)

| Story | Requirements | Status | User story |
|---|---|---|---|
| `DB-001` | `REQ-033` | not-started | TBD - see source audit |
| `DB-002` | - | not-started | Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json |
| `DB-003` | `REQ-006`, `REQ-014` | not-started | TBD - see source audit |
| `DB-004` | - | not-started | Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json |
| `DB-005` | - | not-started | Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json |
| `DB-006` | `REQ-033` | not-started | TBD - see source audit |
| `DB-007` | - | not-started | Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json |
| `DB-008` | `REQ-033` | not-started | TBD - see source audit |
| `DB-009` | `REQ-033` | not-started | TBD - see source audit |
| `DB-010` | - | not-started | Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json |
| `DB-011` | - | not-started | Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json |
| `DB-012` | `REQ-034` | not-started | TBD - see source audit |
| `DB-013` | - | not-started | Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json |
| `DB-014` | - | not-started | Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json |
| `DB-015` | - | not-started | Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json |
| `DB-016` | `REQ-034` | not-started | TBD - see source audit |

### DISC (3 stories)

| Story | Requirements | Status | User story |
|---|---|---|---|
| `DISC-001` | `REQ-003` | not-started | TBD - see source audit |
| `DISC-008` | `REQ-035` | not-started | TBD - see source audit |
| `DISC-010` | `REQ-003` | not-started | TBD - see source audit |

### EXT (12 stories)

| Story | Requirements | Status | User story |
|---|---|---|---|
| `EXT-001` | `REQ-017` | not-started | TBD - see source audit |
| `EXT-002` | `REQ-017` | not-started | TBD - see source audit |
| `EXT-003` | - | not-started | Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json |
| `EXT-004` | - | not-started | Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json |
| `EXT-005` | `REQ-005` | not-started | TBD - see source audit |
| `EXT-006` | - | not-started | Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json |
| `EXT-007` | - | not-started | Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json |
| `EXT-008` | `REQ-020` | not-started | TBD - see source audit |
| `EXT-009` | `REQ-005` | not-started | TBD - see source audit |
| `EXT-010` | - | not-started | Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json |
| `EXT-011` | - | not-started | Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json |
| `EXT-012` | `REQ-005` | not-started | TBD - see source audit |

### INT (10 stories)

| Story | Requirements | Status | User story |
|---|---|---|---|
| `INT-001` | - | not-started | Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json |
| `INT-002` | - | not-started | Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json |
| `INT-003` | - | not-started | Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json |
| `INT-004` | - | not-started | Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json |
| `INT-005` | - | not-started | Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json |
| `INT-006` | - | not-started | Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json |
| `INT-007` | - | not-started | Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json |
| `INT-008` | - | not-started | Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json |
| `INT-009` | - | not-started | Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json |
| `INT-010` | - | not-started | Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json |

### OPS (9 stories)

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

### PROV (12 stories)

| Story | Requirements | Status | User story |
|---|---|---|---|
| `PROV-001` | - | not-started | Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json |
| `PROV-002` | - | not-started | Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json |
| `PROV-003` | - | not-started | Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json |
| `PROV-004` | - | not-started | Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json |
| `PROV-005` | - | not-started | Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json |
| `PROV-006` | - | not-started | Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json |
| `PROV-007` | - | not-started | Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json |
| `PROV-008` | - | not-started | Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json |
| `PROV-009` | - | not-started | Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json |
| `PROV-010` | - | not-started | Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json |
| `PROV-011` | `REQ-010` | not-started | TBD - see source audit |
| `PROV-012` | - | not-started | Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json |

### REL (3 stories)

| Story | Requirements | Status | User story |
|---|---|---|---|
| `REL-001` | `REQ-003` | not-started | TBD - see source audit |
| `REL-002` | `REQ-004`, `REQ-035` | not-started | TBD - see source audit |
| `REL-003` | `REQ-035` | not-started | TBD - see source audit |

### ROUTE (11 stories)

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

### SEC (16 stories)

| Story | Requirements | Status | User story |
|---|---|---|---|
| `SEC-001` | `REQ-021`, `REQ-030` | not-started | TBD - see source audit |
| `SEC-002` | `REQ-025` | not-started | TBD - see source audit |
| `SEC-003` | `REQ-021`, `REQ-029`, `REQ-030` | not-started | TBD - see source audit |
| `SEC-004` | `REQ-027` | not-started | TBD - see source audit |
| `SEC-005` | `REQ-027` | not-started | TBD - see source audit |
| `SEC-006` | `REQ-028`, `REQ-029` | not-started | TBD - see source audit |
| `SEC-007` | `REQ-031` | not-started | TBD - see source audit |
| `SEC-008` | - | not-started | Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json |
| `SEC-009` | - | not-started | Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json |
| `SEC-010` | `REQ-020` | not-started | TBD - see source audit |
| `SEC-011` | `REQ-020` | not-started | TBD - see source audit |
| `SEC-012` | `REQ-025` | not-started | TBD - see source audit |
| `SEC-013` | `REQ-026` | not-started | TBD - see source audit |
| `SEC-014` | `REQ-027` | not-started | TBD - see source audit |
| `SEC-016` | `REQ-025` | not-started | TBD - see source audit |
| `SEC-017` | `REQ-030`, `REQ-035` | not-started | TBD - see source audit |

### SESS (17 stories)

| Story | Requirements | Status | User story |
|---|---|---|---|
| `SESS-001` | `REQ-006` | not-started | TBD - see source audit |
| `SESS-002` | `REQ-023` | not-started | TBD - see source audit |
| `SESS-003` | - | not-started | Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json |
| `SESS-004` | - | not-started | Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json |
| `SESS-005` | - | not-started | Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json |
| `SESS-006` | - | not-started | Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json |
| `SESS-007` | - | not-started | Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json |
| `SESS-008` | - | not-started | Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json |
| `SESS-009` | - | not-started | Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json |
| `SESS-010` | - | not-started | Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json |
| `SESS-011` | `REQ-008` | not-started | TBD - see source audit |
| `SESS-012` | - | not-started | Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json |
| `SESS-013` | - | not-started | Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json |
| `SESS-014` | - | not-started | Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json |
| `SESS-015` | - | not-started | Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json |
| `SESS-016` | - | not-started | Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json |
| `SESS-017` | `REQ-006` | not-started | TBD - see source audit |

### SHARE (5 stories)

| Story | Requirements | Status | User story |
|---|---|---|---|
| `SHARE-001` | `REQ-007` | not-started | TBD - see source audit |
| `SHARE-002` | - | not-started | Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json |
| `SHARE-003` | - | not-started | Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json |
| `SHARE-004` | `REQ-007` | not-started | TBD - see source audit |
| `SHARE-005` | `REQ-007` | not-started | TBD - see source audit |

### TOOL (10 stories)

| Story | Requirements | Status | User story |
|---|---|---|---|
| `TOOL-001` | - | not-started | Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json |
| `TOOL-002` | - | not-started | Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json |
| `TOOL-003` | - | not-started | Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json |
| `TOOL-004` | - | not-started | Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json |
| `TOOL-005` | - | not-started | Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json |
| `TOOL-008` | - | not-started | Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json |
| `TOOL-011` | - | not-started | Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json |
| `TOOL-012` | - | not-started | Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json |
| `TOOL-013` | - | not-started | Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json |
| `TOOL-014` | - | not-started | Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json |

### UI (13 stories)

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

These stories carry `requirementIds: []` in `ralph.json`. They are still mandatory backlog (typically DISC-002 surface-extraction discoveries or not-yet-reconciled scope). Do not treat absence of a requirement link as optional. Task cards in `tasks/` and `feature-ledger.json` may add ownership detail.

- `AGENT-001` (not-started): Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json
- `AGENT-009` (not-started): Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json
- `AGENT-012` (not-started): Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json
- `AGENT-013` (not-started): Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json
- `AGENT-014` (not-started): Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json
- `BASE-001` (not-started): Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json
- `BASE-002` (not-started): Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json
- `BASE-007` (not-started): Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json
- `BASE-008` (not-started): Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json
- `CAT-003` (not-started): Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json
- `CAT-005` (not-started): Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json
- `CAT-006` (not-started): Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json
- `DB-002` (not-started): Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json
- `DB-004` (not-started): Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json
- `DB-005` (not-started): Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json
- `DB-007` (not-started): Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json
- `DB-010` (not-started): Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json
- `DB-011` (not-started): Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json
- `DB-013` (not-started): Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json
- `DB-014` (not-started): Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json
- `DB-015` (not-started): Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json
- `EXT-003` (not-started): Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json
- `EXT-004` (not-started): Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json
- `EXT-006` (not-started): Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json
- `EXT-007` (not-started): Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json
- `EXT-010` (not-started): Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json
- `EXT-011` (not-started): Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json
- `INT-001` (not-started): Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json
- `INT-002` (not-started): Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json
- `INT-003` (not-started): Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json
- `INT-004` (not-started): Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json
- `INT-005` (not-started): Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json
- `INT-006` (not-started): Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json
- `INT-007` (not-started): Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json
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
- `PROV-001` (not-started): Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json
- `PROV-002` (not-started): Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json
- `PROV-003` (not-started): Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json
- `PROV-004` (not-started): Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json
- `PROV-005` (not-started): Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json
- `PROV-006` (not-started): Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json
- `PROV-007` (not-started): Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json
- `PROV-008` (not-started): Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json
- `PROV-009` (not-started): Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json
- `PROV-010` (not-started): Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json
- `PROV-012` (not-started): Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json
- `ROUTE-003` (not-started): Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json
- `ROUTE-004` (not-started): Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json
- `ROUTE-006` (not-started): Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json
- `ROUTE-007` (not-started): Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json
- `ROUTE-009` (not-started): Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json
- `ROUTE-010` (not-started): Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json
- `SEC-008` (not-started): Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json
- `SEC-009` (not-started): Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json
- `SESS-003` (not-started): Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json
- `SESS-004` (not-started): Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json
- `SESS-005` (not-started): Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json
- `SESS-006` (not-started): Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json
- `SESS-007` (not-started): Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json
- `SESS-008` (not-started): Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json
- `SESS-009` (not-started): Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json
- `SESS-010` (not-started): Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json
- `SESS-012` (not-started): Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json
- `SESS-013` (not-started): Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json
- `SESS-014` (not-started): Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json
- `SESS-015` (not-started): Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json
- `SESS-016` (not-started): Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json
- `SHARE-002` (not-started): Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json
- `SHARE-003` (not-started): Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json
- `TOOL-001` (not-started): Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json
- `TOOL-002` (not-started): Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json
- `TOOL-003` (not-started): Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json
- `TOOL-004` (not-started): Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json
- `TOOL-005` (not-started): Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json
- `TOOL-008` (not-started): Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json
- `TOOL-011` (not-started): Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json
- `TOOL-012` (not-started): Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json
- `TOOL-013` (not-started): Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json
- `TOOL-014` (not-started): Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json
- `UI-001` (not-started): Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json
- `UI-002` (not-started): Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json
- `UI-013` (not-started): Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json
- `WEB-001` (not-started): Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json
- `WEB-002` (not-started): Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json
- `WEB-003` (not-started): Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json
- `WEB-004` (not-started): Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json
- `WEB-005` (not-started): Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json
