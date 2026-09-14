# DISC-008 - Exhaustive upstream inventory

Status: COMPLETE. Kind: enabler-or-assurance. Runtime optional: False.
Mandatory for full declared release: yes.

## Outcome

Exhaustive tracked-object inventory of both pinned upstream repos with public
surface catalog. File-level inventory is complete; dynamic registration and
per-file behavior mapping deferred to DISC-003/coverage review.

## Inventory evidence

### opencode @ 95daf90670b7c039c436c85537da5fbfe2205b41

- Tree: b17683f6
- Entries: 6626
- SHA-256: 6121ed118c7c...
- Clean/detached/connectivity: all True
- Role breakdown: source 2244, other 1386, test 1012, docs 784, binary 295,
  migration 193, infra 142, data-or-config 333, config 91, symlink 60,
  generated 35, manifest 41, lockfile 6, license 4

### 9router @ 17c4cc76877bd1755030a8414f8d0083f48dcccf

- Tree: 4a6b1d14
- Entries: 1570
- SHA-256: 5c21b9d1f0b6...
- Clean/detached/connectivity: all True
- Role breakdown: source 905, test 278, docs 134, binary 149

### Output files

- sources/inventory/opencode.jsonl
- sources/inventory/9router.jsonl
- sources/inventory/manifest.json (schemaVersion 3, surfaceRuleSha256 1d2265a196f6)

## Public surface catalog

### CLI commands (~50 unique)

Top-level (yargs `command:` in packages/opencode/src/cli):
acp, agent, attach, auth, config, console, db, debug, export, generate,
github, import, mcp, models, plug, pr, providers, run, serve, session, stats,
tui, uninstall, upgrade, web

Subcommands: add, list, delete, login, logout, switch, open, orgs, install,
track, wait, scrap, v2, file, rg, lsp, skill, snapshot, startup, diagnostics

### Tools (packages/opencode/src/tool/registry.ts)

invalid, task, read, question, todo (todowrite), lsp, plan (exit), webfetch,
websearch, shell (bash), glob, write, edit, grep, apply_patch, skill,
code-mode (gated by flags.experimentalCodeMode), plus plugin-registered custom
tools

### Events (85 names, packages/schema/src/event-manifest.ts)

Families: session 55 (incl session.next.* V2 stream), message 5, permission 4,
tui 4, workspace 3, worktree 2, mcp 2, file 2, question 2, prompt 2,
integration 2, plus agent/command/global/ide/installation/lsp/server/vcs/
todo/plugin 1 each

### Config keys (packages/core/src/config.ts)

Top-level: agents, attachments, autoupdate, commands, compaction, default_agent,
enterprise, experimental, formatter, instructions, lsp, mcp, model, permissions,
plugins, providers, references, share, shell, skills, snapshots, start, stop,
tool_output, watcher

Sub-schemas in packages/core/src/config/*.ts (mcp: 17 keys incl oauth/servers;
agent: 11 keys; compaction: auto/buffer/keep/prune/tokens)

### DB schemas

Core V2 tables (packages/core/src/database/schema.sql.ts): session, message,
part, project, workspace, credential, event, event_sequence, session_input,
session_context_epoch, session_message, migration, __drizzle_migrations, users

Migration dirs: packages/console/core/migrations (85),
packages/stats/core/migrations (8),
packages/effect-drizzle-sqlite/examples/migrations (1). Total 95 SQL files.

### Monorepo packages (32)

app, cli, client, codemode, console, core, desktop, effect-drizzle-sqlite,
enterprise, http-recorder, llm, opencode, plugin, protocol, schema, sdk,
server, session-ui, tui, ui, web, etc.

## Existing discovery data referenced

- sources/surface-candidates.jsonl (32 candidate families, 7592 sources)
- sources/behavior-surface-rules.json (32 rules mapping paths to feature IDs)
- sources/ownership-candidates.json
- sources/disc-002-observed-structure.json (partial, exhaustive:false)
- sources/disc-003-evidence.json + reconciliation
- sources/discovery/DISC-001/source-map.json

## Commands run

```
python3 tools/inventory.py          # exit 0, manifest written
python3 tools/validate_plan.py      # exit 0
```

## Limitations

- Inventory is file-level; dynamic registration and per-file behavior mapping
  deferred to DISC-003 and coverage review
- upstream.lock.json completeness block still says fullCheckoutObtainedHere:false
  but local .upstream/ checkouts verify clean at pinned commits
