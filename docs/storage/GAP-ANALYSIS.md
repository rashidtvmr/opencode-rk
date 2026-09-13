# Storage format-2 vs upstream opencode: gap analysis

Status: read-only analysis. No source file modified. Only this file created.

Method: read our `docs/STORAGE.md`, `docs/storage/CRASH_CONSISTENCY.md`,
`docs/storage/REVIEW.md`, `docs/storage/REVIEW-INDEPENDENT.md`,
`crates/storage/schema/v2/workspace.sql`, `crates/storage/schema/v2/catalog.sql`,
and the upstream reference at `/home/rashid/projects/tmpcodes/opencode`
(commit `07619a09d`; our repo at `ec55912`).
Upstream files read: `packages/opencode/src/session/session.sql.ts`,
`packages/opencode/src/share/share.sql.ts`,
`packages/opencode/src/account/account.sql.ts`,
`packages/opencode/src/project/project.sql.ts`,
`packages/opencode/src/control-plane/workspace.sql.ts`,
`packages/opencode/src/sync/event.sql.ts`,
`packages/opencode/src/storage/schema.sql.ts`,
`packages/opencode/src/storage/schema.ts`,
`packages/opencode/src/storage/db.ts`,
`packages/opencode/src/storage/db.bun.ts`,
`packages/opencode/src/storage/db.node.ts`,
`packages/opencode/src/storage/storage.ts` (JSON key-value store),
`packages/opencode/src/storage/json-migration.ts`,
`packages/opencode/src/session/index.ts` (fork, create, paths),
`packages/opencode/src/session/todo.ts`, `packages/opencode/src/session/compaction.ts`,
`packages/opencode/src/session/message-v2.ts` (attachments),
`packages/opencode/src/permission/index.ts`,
`packages/opencode/src/share/share-next.ts`, `packages/opencode/src/share/session.ts`,
`packages/opencode/src/account/repo.ts`, `packages/opencode/src/global/index.ts`,
plus all ten `packages/opencode/migration/*/migration.sql` journals.
Citation prefixes: `W:<n>` = our `crates/storage/schema/v2/workspace.sql` line n;
`C:<n>` = our `crates/storage/schema/v2/catalog.sql` line n;
`STO:<n>` = our `docs/STORAGE.md`; `CC:<n>` = our `docs/storage/CRASH_CONSISTENCY.md`;
`RIND:<n>` = our `docs/storage/REVIEW-INDEPENDENT.md`;
`U:<path>:<n>` = upstream file and line. "Current upstream code" vs "our planned
behavior" are labeled per row. Nothing here changes the worker contract in our
root `AGENTS.md`.

## 1. Upstream inventory

Upstream persists in ONE sqlite file (`packages/opencode/src/storage/db.ts:33`,
`opencode.db` under `Global.Path.data`, per-channel suffix at `db.ts:31-36`),
plus a legacy JSON key-value store (`storage.ts:231`, `storage/` dir) and flat
secret files. Durability story: `PRAGMA journal_mode=WAL`,
`synchronous=NORMAL`, `busy_timeout=5000`, `cache_size=-64000`,
`foreign_keys=ON`, `wal_checkpoint(PASSIVE)` at `U:db.ts:90-95`. Migrations are
drizzle journals from `packages/opencode/migration/*/migration.sql` applied via
`migrate()` at `U:db.ts:98-113`, with an `OPENCODE_SKIP_MIGRATIONS` flag that
replaces every journal body with `select 1;` at `U:db.ts:107-111`. The bulk
JSON import path (`U:json-migration.ts:49-52`) uses `synchronous=OFF`,
`temp_store=MEMORY`, one `BEGIN TRANSACTION` at `U:json-migration.ts:149` with a
single `COMMIT` at `U:json-migration.ts:403`, and `onConflictDoNothing` inserts
at `U:json-migration.ts:100`.

| # | Upstream table or file | Path and lines | What it stores | Durability story |
|---|---|---|---|---|
| 1 | `session` table | `U:session/session.sql.ts:16-46`, first journal `migration/20260127222353_*/migration.sql` | id, project_id FK cascade, workspace_id, parent_id, slug, directory, title, version, share_url, summary_additions/deletions/files/diffs, revert pointer, permission ruleset JSON, created/updated/compacting/archived times | WAL+NORMAL, FK ON, no byte ceilings, no state CHECKs |
| 2 | `message` table | `U:session/session.sql.ts:48-60` | id, session_id FK cascade, created/updated, opaque `data` JSON (`InfoData`) | Same DB; ordering by `time_created`, cursor index `message_session_time_created_id_idx` from `migration/20260312043431_*/migration.sql` |
| 3 | `part` table | `U:session/session.sql.ts:62-78` | id, message_id FK cascade, denormalized session_id (no FK), opaque `data` JSON (`PartData`, includes file/media/attachment payloads inline as data URLs, see `U:session/message-v2.ts:622-637`) | Same DB; `part_message_id_id_idx` from `migration/20260312043431_*`; attachments live inside JSON, no CAS, no GC |
| 4 | `todo` table | `U:session/session.sql.ts:80-97`, logic `U:session/todo.ts:43-70` | session_id FK cascade, content/status/priority free text, position; PK(session_id, position); full replace-all on every update (`delete` then `insert` in one `Database.transaction`) | Same DB; no status/priority enum constraint; whole-list rewrite |
| 5 | `permission` table | `U:session/session.sql.ts:120-126`, logic `U:permission/index.ts:144-160` | project_id PK FK cascade, persisted approved `Ruleset` JSON; pending ask/reply entries live only in an in-memory `Map` (`U:permission/index.ts:144`), never persisted | Same DB for grants; pending is ephemeral by design |
| 6 | `session_share` table | `U:share/share.sql.ts:5-13`, logic `U:share/share-next.ts:1-120` | session_id PK FK cascade, remote share id, secret, url; sync queue in memory (`U:share/share-next.ts:52-56`) | Same DB for the credential-bearing row; remote sync is best effort |
| 7 | `project` table | `U:project/project.sql.ts:5-16` | id, worktree, vcs, name, icon url/color, times, sandboxes JSON, commands JSON | Same DB; root of session/workspace FK chains |
| 8 | `workspace` table (control plane) | `U:control-plane/workspace.sql.ts:6-17`, journals `20260225215848_*`, `20260303231226_*`, `20260410174513_*` | id, type, name, branch, directory, extra JSON, project_id FK cascade | Same DB; last journal rebuilds the table with `PRAGMA foreign_keys=OFF` |
| 9 | `account`, `account_state`, `control_account` tables | `U:account/account.sql.ts:6-39`, journals `20260213144116_*`, `20260228203230_*`, `20260309230000_*` | RAW `access_token`, `refresh_token`, `token_expiry`, email, url; active account/org pointers | Same DB; secrets in cleartext columns; file fallback `auth.json` at `U:auth/index.ts:9` |
| 10 | `event`, `event_sequence` tables | `U:sync/event.sql.ts:1-16`, journal `20260323234822_*` | per-aggregate `seq` plus typed event rows (`id`, `aggregate_id` FK cascade, `seq`, `type`, `data` JSON) | Same DB; in-process pub/sub augmentation in `U:sync/index.ts`; no global cursor/epoch |
| 11 | JSON key-value store | `U:storage/storage.ts:60-66,220-333` | `storage/session/*`, `message/*`, `part/*`, `todo/*`, `permission/*`, `session_share/*`, `session_diff/*`, `project/*` JSON files; per-file `TxReentrantLock` at `U:storage/storage.ts:225-228`; two historical migrations inside the same file at `U:storage/storage.ts:88-218` | Filesystem only; `writeWithDirs` JSON rewrite per key; migration marker file; superseded by sqlite via `json-migration.ts` |
| 12 | Fork implementation | `U:session/index.ts:517-556`, title helper at `U:session/index.ts:111-117` | Fork = new session row plus deep copy of every message/part row with fresh ascending IDs (`MessageID.ascending`, `PartID.ascending`), parent-ID remap, cutoff at optional messageID; title suffix `(fork #n)` | Inherits SQLite row durability; copies all bytes (no sharing); no provenance column retained |
| 13 | Compaction/summary state | `U:session/compaction.ts:95-365`, `U:session/index.ts:597` (`session_diff` read), `U:storage/storage.ts:199` | Compaction markers as message parts (`type: compaction`), summary counts on session row, `session_diff` JSON blob; plans on filesystem (`U:session/index.ts:235-236`) | Same DB rows plus disposable files; no compaction boundary table |
| 14 | Global paths | `U:global/index.ts:9-26` | XDG data/cache/config/state dirs; single-DB layout (no per-workspace split) | One `opencode.db` for all projects |
| 15 | MCP auth file | `U:mcp/auth.ts:34` | `mcp-auth.json` bearer material beside the DB | Flat file, no broker |

What upstream does NOT durably persist (verified by reading the logic files):
background work is `Effect.forkIn` scope fibers (`U:share/session.ts:48`);
permission pending asks are a memory `Map` (`U:permission/index.ts:144-160`);
share sync queues are memory (`U:share/share-next.ts:52-56`); there is no
admission/inbox table, no approvals ledger, no attempt/tool ledger, no outbox
with cursor semantics, no blob CAS, no retention pins. Our slices 4 and 5
(admission, execution, approvals, retention, outbox) are therefore mostly
superset design with no direct upstream equivalent, compared row by row below.

## 2. Gap table

Statuses: COVERED (our DDL owns it, cited), PARTIAL (structurally present but
named fields or behaviors missing), MISSING (no equivalent), DELIBERATE-DEVIATION
(we consciously chose the safer/bounded design, with contract cite).

| # | Upstream capability | Status in our v2 DDL | Evidence and notes |
|---|---|---|---|
| 1 | Session core record (id, title, timestamps) | COVERED | Our `sessions` at `W:58-79` (id `W:60`, title `W:61`, created/updated `W:72-73`, archived state machine `W:78`). Binary UUID ids per `W:60`; upstream text ids at `U:session/session.sql.ts:18`. |
| 2 | Session project/workspace scoping | PARTIAL | Our `sessions` has no `project_id`/`workspace_id` column; scope comes from file placement (workspace db under catalog registry `C:12-21`, layout `STO:24-32`). Upstream `project_id` FK at `U:session/session.sql.ts:20-23` and `workspace_id` at `U:session/session.sql.ts:23`. Cross-db joins are impossible by layout, which is intentional, but an explicit `workspace_id` check column would aid misplacement detection. |
| 3 | Session slug, directory, version | MISSING | Upstream `slug`, `directory`, `version` at `U:session/session.sql.ts:26-29` (slug used for plan paths at `U:session/index.ts:233-237`). Our `sessions` has none of the three. Planned behavior only; no DDL owner. |
| 4 | Session parent/fork lineage | PARTIAL | Our `fork_parent_id` + `fork_message_seq` at `W:68-69` with pairing CHECK `W:76-77` cover provenance better than upstream `parent_id` alone (`U:session/session.sql.ts:25`), but we do not copy the fork-title rule and we drop `workspace_id` inheritance. Writer follow-up, no DDL change. |
| 5 | Fork deep-copy semantics | DELIBERATE-DEVIATION | Upstream copies every message/part byte row (`U:session/index.ts:525-555`). Our design shares immutable payloads and copies only bounded metadata (`STO:48-54`, `CC:67-68`). Safer on disk; cite worker-contract byte budgets (`AGENTS.md:34-38`). Must not adopt full byte copy. |
| 6 | Message record + ordering | COVERED | Our `messages` at `W:82-97`: explicit `seq` with `UNIQUE(session_pk,seq)` at `W:93`, role/status enums, provider provenance. Upstream orders by wall clock (`message_session_time_created_id_idx`, journal `20260312043431_*`); our seq ordering is the deliberate fix for clock rollback (`STO:42-46`). |
| 7 | Part record (typed kinds) | COVERED | Our `message_parts` at `W:98-109` with bounded `kind`/`ordinal`, payload FK `RESTRICT`. Upstream schemaless `data` JSON at `U:session/session.sql.ts:71`. The JSON flexibility loss is deliberate (byte ceilings `STO:56-63`). |
| 8 | Attachments / file blobs | COVERED | Our `blobs` + `payloads` CAS at `W:21-56` with READY/DELETING/UNAVAILABLE lifecycle, 8 KiB inline boundary (`W:39-40`), tombstone triggers (`W:49-56`). Upstream embeds attachments as data URLs inside part JSON (`U:session/message-v2.ts:622-637`) with no dedup, no GC, unbounded row growth. Ours is a strict superset; do not adopt inline embedding. |
| 9 | Todo persistence | MISSING | Upstream `TodoTable` at `U:session/session.sql.ts:80-97` with replace-all writer at `U:session/todo.ts:43-57`. Our v2 DDL has no todo table or payload root for todos. Needs a feature migration; `STO:17-20` already reserves such migrations. |
| 10 | Persisted permission grants (per project) | MISSING | Upstream `PermissionTable` at `U:session/session.sql.ts:120-126` plus per-session `permission` JSON at `U:session/session.sql.ts:36`. Our `approvals`/`approval_resources` (`W:223-249`) record per-decision evidence, not the reusable grant ruleset. `STO:17-20` names saved permissions as outstanding. |
| 11 | Permission pending lifecycle | PARTIAL | Upstream pending is memory-only (`U:permission/index.ts:144-160`); our `approvals` state enum (`W:231`) persists pending durably, which is a superset. But transitions are unenforced in DDL: zero triggers on `approvals` (`RIND:300-321`), `mandatory_human` flippable, no `resolved_at_us` pairing. Writer-owned per `W:222`, `STO:91-95`. |
| 12 | Share record (id/secret/url) | MISSING | Upstream `SessionShareTable` at `U:share/share.sql.ts:5-13`. Our `retained_payloads` purpose=1 (`W:278`) is a GC pin, not a share credential row; `STO:17-20` explicitly lists sharing-service state as needing its own migration. Secret column needs broker treatment, not plain DDL copy. |
| 13 | Project registry | PARTIAL | Our catalog `workspaces` (`C:12-21`) + `app_settings` (`C:23-28`) cover registry and settings. Missing upstream project fields: `vcs`, `name`, `icon_url/color`, `sandboxes`, `commands` (`U:project/project.sql.ts:6-15`). Enrichment migration, catalog-owned. |
| 14 | Control-plane workspace row | PARTIAL | Our catalog `workspaces.status` lifecycle (`C:17`) covers provisioning/ready/deleting/unavailable vs upstream typeless row (`U:control-plane/workspace.sql.ts:6-17`). Missing `type`/`branch`/`directory`/`extra` passthrough fields. Catalog migration. |
| 15 | Account/credential storage | DELIBERATE-DEVIATION | Upstream stores RAW `access_token`/`refresh_token` in sqlite (`U:account/account.sql.ts:10-12`) plus `auth.json`/`mcp-auth.json` flat files. Our `provider_accounts` keeps only `secret_ref` + generation (`C:33-35`) with broker-owned material and rotation saga (`CC:210-213`). Must not adopt raw secret columns; cite `AGENTS.md:34-38` (no secret logging, no direct secret file access). Routing extras (`priority`, `health`, per-model locks at `C:36-50`) are our superset with no upstream equivalent. |
| 16 | Sync event stream | PARTIAL | Upstream per-aggregate `event`/`event_sequence` (`U:sync/event.sql.ts:1-16`). Our `event_outbox` (`W:293-300`) with global `head/floor/epoch` (`W:16-19`) and prefix-only retention comment (`W:301`) is the designed replacement, but head monotonicity, epoch rotation, and prefix-only pruning are writer-owned, not constraints (`RIND:156-167`, `RIND:276-285`). Outbox intentionally has no FK (`W:295`) so deletion events survive (`RIND:259-274`). |
| 17 | Session summary counts, revert pointer, session_diff | PARTIAL | Upstream `summary_additions/deletions/files/diffs` + `revert` JSON on session (`U:session/session.sql.ts:31-35`) and `session_diff` JSON files (`U:storage/storage.ts:199`, read at `U:session/index.ts:597`). Our `context_epochs` (`W:250-260`) + `compaction_checkpoints` with message-boundary FK (`W:263-273`) model the boundary soundly (`RIND:327-338`) but have no revert pointer, no summary counters, no diff-stream root. Feature migration. |
| 18 | Compacting/archived timestamps | PARTIAL | Our `archived_at_us` with state pairing CHECK (`W:74`, `W:78`) covers upstream `time_archived` (`U:session/session.sql.ts:39`). Upstream `time_compacting` (`U:session/session.sql.ts:38`) has no equivalent; closest is open-epoch row (`W:260`). Minor. |
| 19 | Background-task persistence | COVERED (superset; upstream has none) | Upstream background work is ephemeral fibers (`U:share/session.ts:48`); no background-task table exists upstream. Our `executions.mode` (`W:149`), single-owner guard (`W:162`), and uncertain-state recovery index (`W:165`) durably own what upstream leaves in memory. No adoption needed beyond documenting that `mode=1` rows are the background-task record. |
| 20 | Admission / inbox / promotion receipts | COVERED (superset; upstream has none) | Upstream has no inbox table; messages are written directly. Our `session_inputs` + `session_input_parts` with digest, steer/queue FIFO index (`W:129`), deferred same-session promotion FK (`W:124-125`), and `operation_receipts` (`W:284-292`) are new design with no upstream counterpart. The deferred-FK semantics are verified (`RIND:86-96`). |
| 21 | Tool/attempt ledger with same-session binding | COVERED (superset; upstream has none) | Upstream tool calls live inside part JSON. Our `provider_attempts` (`W:170-184`) and `tool_calls` with composite ownership FKs (`W:205-206`), assistant-role trigger (`W:216-218`), immutable binding trigger (`W:219-220`) are new. Residual: post-terminal output mutability is writer-owned (`RIND:367-371`). |
| 22 | Durability PRAGMAs (WAL+NORMAL, OFF/MEMORY bulk) | DELIBERATE-DEVIATION | Upstream `synchronous=NORMAL` (`U:db.ts:91`) and bulk `synchronous=OFF` + `temp_store=MEMORY` (`U:json-migration.ts:50-52`). Our canonical `synchronous=FULL`, `temp_store=FILE`, `wal_autocheckpoint=1000`, writer-serialized checkpoints (`STO:149-156`), with NORMAL deferred pending barrier proof (`STO:158-159`, `CC:148-150`). Must not adopt; see section 4. |
| 23 | Migration mechanics (drizzle journal, skip flag, FK-OFF rebuild) | PARTIAL | Our `schema_migrations` + checksum fail-closed + `user_version=2` + app-id checks (`STO:143-147`) are stricter than upstream drizzle journals. Missing: a tested bulk-import path with bounded batches (upstream batches at 1000 in `U:json-migration.ts:71`), and an explicit decision record that the skip-migrations hatch and FK-OFF rebuild will never be adopted (section 4). |

## 3. Upstream behaviors we should adopt

Ordered by risk (lowest risk first). Each item names the exact upstream evidence
and the file to change, with no code written here.

1. Approvals `resolved_at_us` pairing CHECK. What: add
   `CHECK((state=0 AND resolved_at_us IS NULL) OR (state<>0 AND resolved_at_us IS NOT NULL))`
   mirroring `W:160`/`W:208`. Why: sibling tables enforce terminal bookkeeping in
   SQL while approvals rely on prose; asymmetry documented at `RIND:300-321`.
   Evidence: `U:session/session.sql.ts:120-126` (zero CHECKs upstream, so this is
   our hardening, not parity). Change: `crates/storage/schema/v2/workspace.sql`
   approvals block plus contract tests. Risk: low, new-file DDL only.
2. `mandatory_human` immutability trigger. What: freeze `mandatory_human` after
   insert so a later flip cannot satisfy `W:239` without a human. Why: probe
   shows `1 to 0` flip accepted (`RIND:304-305`). Evidence: upstream has no such
   column at all, so again hardening. Change: same file, approvals triggers.
   Risk: low.
3. Blob quarantine transition guard. What: writer rule that `0 to 2`
   (READY to UNAVAILABLE) requires a quarantine reason and `2 to 0` requires
   full revalidation before serving reads. Why: both transitions unconstrained
   in DDL (`RIND:64-66`, `RIND:232-238`); `CC:146` promises references preserved
   for diagnostics. Evidence: upstream has no blob lifecycle, so this is new
   writer scope. Change: writer module + tests, no DDL. Risk: low.
4. GC reachability via per-arm `NOT EXISTS`. What: writer GC uses correlated
   per-arm seeks over covering indexes instead of `NOT IN (payload_roots view)`.
   Why: the view materializes all roots per pass (`RIND:355-363`); indexes
   already exist (`W:109`, `W:142`, `W:166`, `W:213-215`, `W:261-262`,
   `W:274-275`, `W:283`). Evidence: upstream has no GC at all. Change: writer
   query module + bench. Risk: low-medium (perf only, correctness covered by
   `payload_roots` at `W:302-313`).
5. Fork-title convention as writer rule. What: `(fork #n)` suffix increment per
   `U:session/index.ts:111-117`, applied when `fork_parent_id` is set.
   Why: cheapest upstream parity with user-visible benefit; zero DDL impact.
   Change: writer/session service + test. Risk: low.
6. Todo table feature migration. What: session-scoped todos modeled on upstream
   `TodoTable` (`U:session/session.sql.ts:80-97`): content/status/priority with
   position ordering, cascade on session delete, plus a payload-root arm if todo
   bodies reference CAS. Why: only upstream sqlite table with zero v2 coverage
   (row 9); `STO:17-20` reserves exactly this migration class. Change: new
   migration file + `payload_roots` extension + writer. Risk: medium (new
   writes, small blast radius).
7. Project registry enrichment. What: `vcs`, `name`, icon, `sandboxes`,
   `commands` from `U:project/project.sql.ts:6-15` into catalog
   (`workspaces` columns or `app_settings` keys). Why: importer fidelity;
   current catalog drops them (row 13). Change: `crates/storage/schema/v2/catalog.sql`
   migration + importer mapping. Risk: medium (catalog-owned, additive).
8. Session descriptor fields (`slug`, `directory`, `version`). What: add the
   three columns from `U:session/session.sql.ts:26-29` to `sessions` with byte
   ceilings in our style. Why: plan-path derivation (`U:session/index.ts:233-237`)
   and version gating need them; currently MISSING (row 3). Change: workspace
   migration + writer. Risk: medium (touches the hottest table; additive NULLable).
9. Revert pointer + summary counters. What: `revert` reference and
   summary add/del/files from `U:session/session.sql.ts:31-35` as a
   compaction-adjacent record bound to `compaction_checkpoints`. Why: revert and
   summary UX have no v2 owner (row 17). Change: workspace migration + writer.
   Risk: medium.
10. Persisted grant ruleset + share record (broker-treated). What: project/session
    permission ruleset (`U:session/session.sql.ts:36,120-126`) and share row
    (`U:share/share.sql.ts:5-13`) as NEW feature migrations with `secret_ref`
    hygiene per `CC:210-213`, never raw secrets. Why: `STO:17-20` names both as
    outstanding; naive copy would violate the credential boundary. Change: new
    migrations + broker integration + tests. Risk: medium-high (auth-adjacent,
    needs threat review first).
11. Bounded bulk-import path. What: importer batching discipline (1000-row
    batches in `U:json-migration.ts:71`) redone under our barriers: FULL,
    bounded transactions, progress + resumability, count/hash verification per
    `STO:199-201`. Why: activation needs an import story and upstream proves the
    batch size works, but its `OFF/MEMORY/single-COMMIT` envelope (`row 22`)
    must not be copied. Change: importer module, not the DDL. Risk: high
    (touches real user data; read-only source to NEW destination only).
12. Head/epoch/pruning writer protocol + tests. What: head monotonicity,
    `cursor_epoch` rotation on restore, contiguous-prefix pruning (`RIND:156-167`,
    `RIND:280-285`, `CC:190-201`). Why: three advertised invariants with no SQL
    owner; outbox resync depends on them (`STO:184-188`). Change: writer + fault
    tests. Risk: high (correctness-critical, concurrency-adjacent).

## 4. Upstream behaviors we must NOT adopt

1. `synchronous=NORMAL` steady state and `synchronous=OFF` + `temp_store=MEMORY`
   bulk mode (`U:db.ts:91`, `U:json-migration.ts:49-52`). Reason: our canonical
   FULL default (`STO:151-152`, `STO:158-159`) exists because NORMAL can lose a
   committed reference deletion after power loss while an unlink stayed durable
   (`CC:148-150`). Adopting NORMAL/OFF would trade user transcripts for import
   speed and violate the worker contract against silent durability downgrades
   and unbounded temp memory discipline (root `AGENTS.md:34-38` resource bounds;
   `STO:159-160` max_page_count caveat).
2. Raw `access_token`/`refresh_token` columns and flat secret files
   (`U:account/account.sql.ts:10-12`, `U:auth/index.ts:9`, `U:mcp/auth.ts:34`).
   Reason: our `secret_ref` + broker rotation saga (`C:33-35`, `CC:210-213`)
   exists to keep raw material out of sqlite, logs, and backups. Copying the
   columns violates `AGENTS.md:34-38` (no secret logging, no direct secret file
   access, explicit capabilities for credentials).
3. Unbounded schemaless `data` JSON columns as the canonical store
   (`U:session/session.sql.ts:57,71`, `U:session/session.sql.ts:111-126`
   commented entry table). Reason: every free text/JSON field in v2 carries a
   byte ceiling (`STO:56-63`) and cell caps are paired with admission quotas;
   unbounded JSON reintroduces the unbounded-retained-output class the worker
   contract forbids (`AGENTS.md:34-38`).
4. `OPENCODE_SKIP_MIGRATIONS` content blanking (`U:db.ts:107-111`). Reason: our
   checksum fail-closed boot (`STO:145-147`, verified `RIND:382-386`) must never
   gain a flag that stamps a false version. Violates the authority rule against
   mutating version/acceptance state (`AGENTS.md:10-15`).
5. `PRAGMA foreign_keys=OFF` table rebuilds (journal
   `migration/20260410174513_workspace-name/migration.sql`). Reason: v2 is
   new-file-only DDL in one transaction (`W:1-3`, `STO:149-150`); live rebuilds
   with enforcement off contradict the FK-supported cascade/index analysis
   (`RIND:240-256`) and the no-clobber rule for existing user data
   (`AGENTS.md:45-47`, `STO:199-201`).
6. Whole-list todo rewrite without bounds (`U:session/todo.ts:43-57` delete-all
   then insert). Reason: acceptable upstream at small scale, but our writer must
   bound the replacement batch and validate enum values; unbounded single-txn
   rewrites risk WAL spikes against the quota policy (`STO:56-63`).
7. Byte-copy fork as the only fork primitive (`U:session/index.ts:525-555`).
   Reason: duplicates attachment bytes per fork with no sharing; our
   metadata-only promotion/fork over shared payloads (`STO:48-54`) is the
   bounded replacement. Copy semantics may be offered as an explicit export,
   never as the default fork path.

## 5. Verdict

For the stated slice (sessions, messages, admission, execution, approvals,
retention, outbox) the v2 DDL is a safe SUPERSET of upstream: every upstream
capability in that slice has a cited v2 owner (rows 1, 4, 6, 7, 8, 11, 16,
19, 20, 21), and where we diverge (seq ordering, CAS, tombstones, FULL,
deferred same-session FKs) the deviation is toward the safer design with a
named contract cite. Outside the slice the DDL is a deliberate SUBSET: todos,
saved grants, share credentials, project enrichment, session descriptors, and
revert/summary state are MISSING or PARTIAL (rows 3, 9, 10, 12, 13, 14, 17),
exactly as `STO:17-20` discloses, and each has a migration-shaped follow-up in
section 3. No upstream table was found that contradicts the v2 ownership model;
the sharpest structural differences (single global `opencode.db` at
`U:global/index.ts:9-26` vs per-workspace DBs plus catalog at `STO:24-32`;
wall-clock ordering vs explicit seq; inline attachments vs CAS) all resolve in
v2's favor on boundedness and crash semantics, at the cost of importer work.

Top 3 residual risks:

1. Quotas have no owner anywhere (MISSING, `RIND:178-184`). Cell ceilings
   (`W:39-40` and siblings) are not disk totals; DB/WAL/CAS/staging/cache/backup
   accounting plus admission backpressure exist only as prose (`STO:56-63`).
   Without them a hostile or merely large workload can fill the disk while every
   SQL CHECK still passes. Owners: writer (admission caps, staged-bytes
   budgets) plus operator (filesystem quotas, monitoring). Blocks activation.
2. Approval authority lives in the writer, not the rows (`RIND:300-321`, row 11).
   Pending-to-terminal transitions, `mandatory_human` freezing, expiry
   evaluation, and `resolved_at_us` pairing are unenforced until section 3 items
   1-2 and the writer state machine land. A buggy broker could resurrect a
   consumed grant or drop the human binding with DDL consent. Owner: writer
   (typed API + transition tests); SQL owns only the identity CHECK at `W:239`.
3. Cursor and pruning discipline is writer-owned on a multilateral protocol
   (`RIND:156-167`, row 16; `CC:190-201`). Lowered head, mutated epoch, or
   hole-punching deletes are DDL-legal and would silently force mass RESYNC or,
   worse, skip events for consumers that trust the watermark. Owner: writer
   (monotonicity guards, prefix-only pruning, restore rotation) plus operator
   (engine gate `STO:163-168`, currently failing closed per `RIND:209-220`, so
   activation is correctly blocked until the bundle is remediated).
