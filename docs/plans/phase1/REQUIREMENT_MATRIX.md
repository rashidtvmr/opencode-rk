# Requirement disposition matrix

All 47 observed original requirement IDs are retained (the source has no REQ-039). Summaries are restatements, not verbatim quotations. Every scope split is proposed and pending approval; no full-release requirement is deleted.

| Requirement | Restated scope | Packages | Proposed disposition |
|---|---|---|---|
| REQ-001 | Rust/Tokio and practical resource efficiency | P1-W1-08, P1-W5-07 | included |
| REQ-002 | Ralph plan, independent slices and autonomous loop | P1-W1-01, P1-W5-08 | split_proposed / SCOPE-07 |
| REQ-003 | Every OpenCode V2 feature accounted for | P1-W1-10, P1-W5-09 | included |
| REQ-004 | Strict TDD and independent verification | P1-W1-05, P1-W1-10, P1-W6-09 | included |
| REQ-005 | OpenCode V2 plugins including UI behavior | P1-W4-05, P1-W1-01 | split_proposed / SCOPE-02 |
| REQ-006 | Session management and persistent history | P1-W2-09, P1-W3-01 | included |
| REQ-007 | Session sharing | P1-W3-09 | split_proposed / SCOPE-10 |
| REQ-008 | Session forking | P1-W3-02 | included |
| REQ-009 | Multiple delegation types and custom agents | P1-W4-01 | included |
| REQ-010 | Cross-provider subagent delegation | P1-W4-01, P1-W4-02 | included |
| REQ-011 | Searchable models.dev-backed API | P1-W3-05, P1-W4-02 | included |
| REQ-012 | Foreground and background subagents | P1-W4-01 | included |
| REQ-013 | Navigate, resume and steer subagents | P1-W3-06, P1-W4-01 | included |
| REQ-014 | Subagent context and messages in database | P1-W4-01, P1-W2-09 | included |
| REQ-015 | Singleton serving multiple clients | P1-W1-07, P1-W2-09 | included |
| REQ-016 | Status panel | P1-W3-07 | included |
| REQ-017 | Skills, plugins and custom slash commands | P1-W4-05 | split_proposed / SCOPE-02 |
| REQ-018 | Independent main/child effort | P1-W3-05, P1-W4-01 | included |
| REQ-019 | Themes and common settings | P1-W3-07 | included |
| REQ-020 | Pre/post tool hooks | P1-W4-05, P1-W5-05 | included |
| REQ-021 | Permissions and mandatory human control | P1-W2-06, P1-W5-04 | included |
| REQ-022 | Built-in multi-account routing inspired by 9router | P1-W4-02 | included |
| REQ-023 | Response timestamps | P1-W3-03, P1-W4-07 | included |
| REQ-024 | Lightweight delegation on basic host/VPS | P1-W4-01, P1-W5-07 | included |
| REQ-025 | Deterministic destructive-command and SQL controls | P1-W2-06, P1-W5-04 | included |
| REQ-026 | Manual-only destructive instructions | P1-W2-06, P1-W5-04 | included |
| REQ-027 | OS-enforced sensitive-file and environment restrictions | P1-W5-04 | included |
| REQ-028 | Safe system-file reads without agent modification | P1-W5-04 | included |
| REQ-029 | Trusted user destructive-protection toggle | P1-W2-06, P1-W3-07 | included |
| REQ-030 | Permission wildcard cannot bypass mandatory controls | P1-W2-06, P1-W5-04 | included |
| REQ-031 | Explicit approval for other projects/user files | P1-W2-06, P1-W5-04 | included |
| REQ-032 | Configurable features with no hidden runtime cost when off | P1-W3-07, P1-W5-07 | included |
| REQ-033 | Embedded scalable storage without external database install | P1-W2-09, P1-W6-07 | included |
| REQ-034 | Safe large OpenCode history import | P1-W3-09, P1-W6-07 | included |
| REQ-035 | Safety and resource correctness over literal translation | P1-W1-04, P1-W5-04, P1-W5-07 | included |
| REQ-036 | Full child lifecycle, models, compression, handoff and settings | P1-W3-06, P1-W4-01 | included |
| REQ-037 | Typed tool contract, diagnostics and review discipline | P1-W2-05, P1-W3-07 | included |
| REQ-038 | Redacted provider observability and debug export | P1-W4-02, P1-W5-05 | included |
| REQ-040 | Local ChatGPT-class web experience including research, voice, Library and artifacts | P1-W4-06, P1-W4-07, P1-W4-08, P1-W4-09, P1-W5-01, P1-W5-02 | included |
| REQ-041 | Documented provider compatibility, OAuth/consent-based import and usage telemetry | P1-W1-09, P1-W2-04, P1-W3-05, P1-W4-02 | included_with_authority_blockers / SCOPE-09 |
| REQ-042 | Operational panel and searchable/toggleable MCP lifecycle | P1-W3-07, P1-W4-03 | included |
| REQ-043 | Versioned sync/replay and durable versus ephemeral events | P1-W2-09, P1-W5-06 | included |
| REQ-044 | Per-session busy rejection, shell lane and owned cancellation | P1-W2-05, P1-W5-06 | included |
| REQ-045 | ACP v1 JSON-lines bridge with explicit limits | P1-W1-08 | scope_review_later_proposed / SCOPE-11 |
| REQ-046 | Workspace HTTP/WebSocket proxy and remote sync | P1-W1-08, P1-W2-09 | split_proposed / SCOPE-11 |
| REQ-047 | Typed SDK and process/TUI spawners with directory scoping | P1-W1-08, P1-W1-07 | split_proposed / SCOPE-11 |
| REQ-048 | Headless execution and session export | P1-W1-08, P1-W3-09 | included |
