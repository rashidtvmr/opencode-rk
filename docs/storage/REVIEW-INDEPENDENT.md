# Independent adversarial review: format-2 storage DDL

Reviewer: independent subagent (adversarial DDL review role). Not the design
author. Read-only on source; only this markdown file was created.

Date: 2026-09-13. Base commit: `ec559120c4c5a27a1195150904b38f04463d0325`
(`feat(storage): define format-2 schemas and crash-consistency contracts`).
Reviewed tree: that commit plus the in-flight uncommitted implementation wave
(`crates/storage/src/schema_v2.rs`, `crates/storage/tests/writer_v2.rs` are
untracked; `crates/storage/src/lib.rs` has a 2-line export diff).

Citation shorthand: `W:<n>` = `crates/storage/schema/v2/workspace.sql` line n;
`C:<n>` = `crates/storage/schema/v2/catalog.sql` line n; `REV:<n>` =
`docs/storage/REVIEW.md` line n; `SCHEMA_INIT:<n>` =
`crates/storage/src/schema_v2.rs` line n; `WRITER_RED:<n>` =
`crates/storage/tests/writer_v2.rs` line n; `PY:<n>` =
`tests/bootstrap/test_storage_schema_v2.py` line n; `RS:<n>` =
`crates/storage/tests/schema_v2.rs` line n; `CC:<n>` =
`docs/storage/CRASH_CONSISTENCY.md` line n; `STO:<n>` = `docs/STORAGE.md` line n.

Method: every claim below was checked against the exact SQL text and, where
behavior was in doubt, probed empirically on the local engine (Python 3.14.4,
SQLite 3.46.1) by executing the exact DDL from `crates/storage/schema/v2/` and
issuing adversarial statements. Probe outputs are quoted inline. Probes are
evidence for SQLite 3.46.1 only and are not production qualification, per the
engine gate at STO:163-168.

## Part 1: verdict table for the 16 REVIEW.md findings rows

Ratings: CONFIRMED (SQL object exists at cited line and behavior verified),
PARTIAL (object exists but part of the claim has no owner in the DDL),
MISSING (no SQL/runtime object in this wave owns the claim).

| # | REV line | Claim | Owner object | Rating |
|---|---|---|---|---|
| 1 | REV:21 | DELETING tombstone until unlink finishes | `blobs.state` CHECK W:24 + triggers W:49-54 | CONFIRMED |
| 2 | REV:22 | Retention leases + fresh reachability | `payload_roots` view W:302-313, pins W:276-283; leases runtime-only | PARTIAL |
| 3 | REV:23 | Canonical FULL, weaker profile deferred | No DDL object; policy STO:151-152, STO:158-159; init code SCHEMA_INIT:157-168 | PARTIAL |
| 4 | REV:24 | Deferred same-session NO ACTION promotion FK | `session_inputs` FK W:124-125 + parent key W:94 | CONFIRMED |
| 5 | REV:25 | Shared immutable payloads, metadata-only promotion | `payloads` immutability W:47-48, W:38-40; part FKs W:102, W:137 | CONFIRMED |
| 6 | REV:26 | Tools cannot cross sessions | Composite FKs W:205-206 + triggers W:216-220 | CONFIRMED |
| 7 | REV:27 | Binary external IDs, integer internal keys | BLOB(16) id CHECKs W:60, W:84, W:114, W:146, W:172, W:188, W:225; STRICT | CONFIRMED |
| 8 | REV:28 | Reuse equivalent UNIQUE keys | No redundant DESC index; UNIQUE(session_pk,seq) W:93 serves history; map STO:104-121 | CONFIRMED |
| 9 | REV:29 | Authenticated retry horizon, reject expired | `retry_until_us > created_at_us` W:290 + index W:292; expiry check runtime-only | PARTIAL |
| 10 | REV:30 | head/floor/epoch, contiguous prefix | CHECK W:19 (floor bound); prefix-only rule is a comment W:301, not a constraint | PARTIAL |
| 11 | REV:31 | Uncertain state keeps gate closed | `state BETWEEN 0 AND 5` W:150 + partial UNIQUE W:162 | CONFIRMED |
| 12 | REV:32 | Independent quotas (DB/WAL/CAS/staging/cache) | None in DDL; policy text STO:56-63 only | MISSING |
| 13 | REV:33 | FILE temp, checkpoint safety net | `temp_store=FILE` in init configure SCHEMA_INIT:164; safety net absent | PARTIAL |
| 14 | REV:34 | Validate header/digest/length before adoption | Registry-side W:43-46 (state+length); header/digest validation is runtime CC:105-108 | PARTIAL |
| 15 | REV:35 | Snapshot-specific backup pins | `retained_payloads` purpose=2 W:276-283, root view W:313 | CONFIRMED |
| 16 | REV:36 | Fixed engine qualification gate | Decision STO:163-168; measured bundle 3.50.2 FAILS the gate | CONFIRMED (gate holds, blocks activation) |

## Part 2: row-by-row evidence

### Row 1 (REV:21): blob tombstone
CONFIRMED. `blobs.state IN (0,1,2)` at W:24 with the 1=DELETING semantics
comment. `blob_gc_claim` W:49-51 aborts collecting a referenced blob;
`blob_no_resurrection` W:52-54 aborts leaving DELETING to any other state;
`payload_ready` W:43-46 aborts referencing anything but a READY blob with
matching raw length. Empirical: `UPDATE blobs SET state=0` on a DELETING row is
rejected with `finish deletion before republishing`; `DELETE FROM blobs` on a
referenced blob is rejected (RS:90, PY:103). Deleting an unreferenced DELETING
tombstone is allowed, which GC step 4 (CC:138-139) requires.
Gap noted in Part 3(a-adjacent): the READY(0) to UNAVAILABLE(2) and back to
READY(0) transitions are unconstrained (probe: both accepted). Quarantine and
revalidation are writer-owned; only the DELETING boundary is protected.

### Row 2 (REV:22): leases plus fresh reachability
PARTIAL. The DDL owners are the `payload_roots` view (W:302-313) and
`retained_payloads` pins (W:276-283, partial FK-support index W:283). The
retention lease itself is a runtime contract with no SQL object anywhere in
this wave (CC:33-41). The view supports a fresh transaction-time check
(CC:127-129) but nothing in the DDL creates, holds, or expires a lease.

### Row 3 (REV:23): canonical FULL
PARTIAL. No DDL object exists; connection policy is STO:151-152 and the
deferred-NORMAL rationale is STO:158-159 and CC:148-150. In the in-flight
implementation, `open_existing` applies `synchronous=FULL` (SCHEMA_INIT:160)
but `initialize_workspace` does NOT apply the configure block at all
(SCHEMA_INIT:41-87 uses only `PRAGMA_BODY` at SCHEMA_INIT:20); see Part 5.
The bundled engine defaults synchronous to 2 (FULL) per
`libsqlite3-sys-0.35.0/sqlite3/sqlite3.c:17833-17837`, so the missing PRAGMA
is masked, but `foreign_keys` defaults to OFF and is genuinely missing on the
fresh connection (Part 5, implementation defect).

### Row 4 (REV:24): promotion FK
CONFIRMED. `FOREIGN KEY(promoted_message_pk, session_pk) REFERENCES
messages(pk, session_pk) ON DELETE NO ACTION DEFERRABLE INITIALLY DEFERRED`
at W:124-125. Parent key exists as `UNIQUE(pk, session_pk)` at W:94, which
satisfies the SQLite parent-key rule for composite FKs. Empirical on 3.46.1:
inserting a promoted input whose `promoted_message_pk` matches no message is
accepted at statement time and rejected at COMMIT with
`FOREIGN KEY constraint failed` (deferred semantics work as claimed). The
promoted-state CHECK at W:126-127 keeps state and receipt columns paired, so
SET NULL could not satisfy it - the NO ACTION choice in REV:24 is consistent.

### Row 5 (REV:25): shared payloads
CONFIRMED. `payloads` rows are immutable (trigger W:47-48, test RS:94-104),
carry exactly one representation (W:38), and inline length must equal
`raw_bytes` and stay <=8192 (W:39-40). Parts reference payloads by integer
`payload_pk` (W:102, W:137) with `ON DELETE RESTRICT`, so promotion copies
references, not bytes; PY:122-134 proves the payload count stays 1 across a
promotion and the session delete leaves `PRAGMA foreign_key_check` clean.

### Row 6 (REV:26): tool same-session binding
CONFIRMED. INSERT side: composite FKs
`FK(execution_pk, session_pk) REFERENCES executions(pk, session_pk)` W:205 and
`FK(assistant_message_pk, session_pk) REFERENCES messages(pk, session_pk)`
W:206, plus trigger `tool_assistant_owner` W:216-218 which additionally
requires the owner message to have role=2 (assistant). UPDATE side: trigger
`tool_binding_immutable` W:219-220 fires `BEFORE UPDATE OF id, session_pk,
execution_pk, assistant_message_pk, ordinal, provider_call_id, name,
intent_hash, input_payload_pk`. This covers the specific question raised:
line 216-218 alone is INSERT-only, but every binding column including
`assistant_message_pk` and `session_pk` is in the line 219 column list, so
repointing is blocked. Empirical on 3.46.1: all three attempted repoints
(repoint to a user-role message in the same session, repoint execution to
another session's execution, flip session_pk) each failed with
`tool invocation binding is immutable`. The assistant-role requirement on
UPDATE is enforced transitively: since assistant_message_pk cannot change,
the role=2 invariant established at insert cannot be lost. Cross-session
INSERT is caught by the composite FKs (PY:156-162).
Residual gap (not a REV claim): `output_payload_pk` and `error_payload_pk`
are deliberately absent from the immutable list (they are set after dispatch);
empirically `output_payload_pk` is mutable even after a terminal state, so
terminal-state immutability is writer-owned per STO:94-95. Recorded in Part 4.

### Row 7 (REV:27): binary IDs
CONFIRMED. Every public id is `BLOB NOT NULL ... CHECK(length(id) = 16)`
(W:60 sessions, W:84 messages, W:114 inputs, W:146 executions, W:172 attempts,
W:188 tool_calls, W:225 approvals, W:285 receipts, C:14 workspaces, C:31
accounts) and joins use integer `pk` rowids. STRICT makes the BLOB type
binding real: probes inserting TEXT into `blobs.hash` and an INTEGER into
`messages.id` both failed with `cannot store ... value in BLOB column`.

### Row 8 (REV:28): index dedup
CONFIRMED. Message history reverse scans reuse `UNIQUE(session_pk, seq)`
(W:93); there is no separate descending index over it. The two sessions
indexes (W:80-81) are not duplicates: one is state-filtered, the other is not.
The EXPLAIN contract test PY:236-245 asserts the four representative access
paths use the intended indexes with no `TEMP B-TREE`. The deliberate extra
`UNIQUE(pk, session_pk)` on messages (W:94) exists as the composite FK parent
key and is acknowledged in the index map STO:111; it is added cost, not
duplication.

### Row 9 (REV:29): retry horizon
PARTIAL. `retry_until_us > created_at_us` CHECK at W:290 and expiry scan
index `receipts_expiry_idx(retry_until_us, operation_id)` at W:292 exist and
work (probe: expiry lookup uses the covering index). But nothing in the DDL
rejects a retry presented after `retry_until_us`; the authenticated
issuance/expiry token check is explicitly a writer step (STO:180-182:
"validate a trusted authenticated issuance/expiry token BEFORE lookup"). The
DDL alone cannot express it. Missing piece is runtime, by design, but the
REV:29 wording ("reject expired retries") has no SQL owner in this wave.

### Row 10 (REV:30): head/floor/epoch
PARTIAL. `CHECK(0 <= event_floor_seq AND event_floor_seq <= event_head_seq)`
at W:19 is real: probes confirmed floor>head rejected, negative floor
rejected, negative head rejected (the CHECK covers head transitively).
Outbox seq is a positive INTEGER PRIMARY KEY (W:294) so allocation never
reuses a rowid. However: (1) "deletes only a sequence prefix" is a comment at
W:301, not a constraint - `DELETE FROM event_outbox WHERE seq=5` (a hole) is
DDL-legal; (2) head monotonicity is unconstrained - empirically lowering
`event_head_seq` from 5 to 3 with floor 0 passes the CHECK; (3) `cursor_epoch`
is freely mutable (probe: UPDATE accepted, no trigger). All three are assigned
to writer discipline, so the invariant set actually owned by SQL is smaller
than the row implies.

### Row 11 (REV:31): uncertain gate
CONFIRMED. Execution states 0-5 include 4=uncertain (W:150 comment); partial
unique index `executions_single_owner_idx ... WHERE state IN (0,1,4)` (W:162)
keeps one queued/running/uncertain execution per session, so an uncertain run
blocks a replacement. Verified by PY:145-149 (uncertain blocks new run,
transition to cancelled state 5 frees the slot). Finished_at consistency is
enforced at W:160. Tool-side uncertainty (state 5) participates in
`tools_recovery_idx` W:212.

### Row 12 (REV:32): quotas
MISSING. No SQL object and no runtime object in this wave bounds DB, WAL,
CAS, staging, cache or backup totals. STO:56-63 states the policy ("These are
NOT total memory/disk limits") and REVIEW.md itself lists this under "not
certified" (REV:46-51). The row records a decision, but in the DDL there is
nothing to confirm; max_page_count is explicitly not a WAL/blob cap
(STO:159-160). This must stay a release gate.

### Row 13 (REV:33): temp and checkpoints
PARTIAL. `temp_store=FILE` is applied by the initializer's configure block
(SCHEMA_INIT:164) and the policy is STO:152-154 with `wal_autocheckpoint=1000`
and writer-serialized checkpoints (STO:154). No checkpoint safety net,
backpressure or bounded-reader cancellation exists anywhere yet; CC:184-186 is
design text. Rating PARTIAL because the connection policy exists as code but
the safety net does not.

### Row 14 (REV:34): validate before adoption
PARTIAL. What SQL can check, it checks: `payload_ready` W:43-46 requires the
referenced blob to exist, be READY (state=0), and have `raw_bytes` equal to
the payload's. The immutable header digest (BLAKE3 with domain prefix,
52-byte header, CC:76-92) and filesystem validation before adoption
(CC:105-108) are runtime steps with no DDL presence. `verified_at_us` on
blobs (W:29) is an unconstrained advisory column; nothing ties it to state.

### Row 15 (REV:35): backup pins
CONFIRMED. `retained_payloads` with `purpose IN (0,1,2)` (2=backup pin) at
W:278, PK W:281, FK RESTRICT W:279, GC-support index W:283, and inclusion in
`payload_roots` W:313. Test PY:187-191 proves a pinned payload remains a root
and cannot be deleted. The snapshot-time shared lease (CC:190-197) is
runtime, but the DDL pin mechanism the row claims does exist and works.

### Row 16 (REV:36): engine qualification gate
CONFIRMED, and it currently BLOCKS activation. STO:163-168 requires bundled
SQLite >=3.51.3 or an audited fixed backport (3.44.6/3.50.7 carry the WAL
reset fix). Measured: the workspace pins `rusqlite 0.37` with
`features=["bundled"]` (`Cargo.toml:27`), which resolves to
`libsqlite3-sys 0.35.0` (`Cargo.lock:741`), whose bundled amalgam declares
`SQLITE_VERSION "3.50.2"`
(`~/.cargo/registry/src/index.crates.io-.../libsqlite3-sys-0.35.0/sqlite3/sqlite3.h:149`).
3.50.2 < 3.50.7, so the gate is not met; local Python SQLite is 3.46.1,
also below. No dependency is changed in this wave, which is the correct
fail-closed posture; the consequence is that format-2 activation cannot
proceed on the current bundle.

## Part 3: directed defect hunt

### (a) STRICT tables with BLOB length CHECKs
No defect found. STRICT applies column type enforcement before CHECKs
evaluate; probes inserting TEXT into `payloads.inline_data`, TEXT into
`blobs.hash`, and INTEGER into `messages.id` all failed with type errors, so
`length()` CHECKs only ever see properly typed BLOBs. The byte-length
convention is correct and consistent: BLOB columns use `length(col)` directly
(W:23, W:6), while TEXT columns use `length(CAST(col AS BLOB))` to count
bytes not characters (W:61, W:63-65, W:103-104, etc.). The boolean-arithmetic
CHECK `(inline_data IS NOT NULL) + (blob_pk IS NOT NULL) = 1` (W:38) is valid
under STRICT because TRUE/FALSE are INTEGER 1/0. Minor observation, not a
defect: `blobs` state transitions 0 to 2 and 2 to 0 (quarantine and
unquarantine) are unconstrained by any trigger (probe: both accepted); only
the DELETING boundary is guarded. CC:146 says corrupt READY becomes
UNAVAILABLE "with references preserved" - the return path 2 to 0 without
revalidation is writer discipline.

### (b) WITHOUT ROWID tables with ON DELETE CASCADE
No performance trap found. The five WITHOUT ROWID tables are `message_parts`
(W:107-108), `session_input_parts` (W:140-141), `approval_resources`
(W:248-249), `retained_payloads` (W:281-282), `operation_receipts`
(W:291) plus catalog `app_settings` (C:28) and
`provider_account_model_locks` (C:49). Verified via `pragma_index_list`
origin=pk on all seven. Cascade discovery probes: deleting a session plans
seeks into every child through covering indexes (messages via
`sqlite_autoindex_messages_2`, inputs, executions, approvals, context,
compaction), no child table scan; `message_parts` by `message_pk` uses the
PRIMARY KEY prefix (WITHOUT ROWID PK seek); catalog
`provider_account_model_locks` by `account_pk` uses PRIMARY KEY prefix
(C:48 PK(account_pk, model_id)). The one non-prefix FK in a WITHOUT ROWID
table, `retained_payloads.payload_pk` (third PK column), has the dedicated
`retained_payload_idx` (W:283) - the exact index that would otherwise be
missing. `operation_receipts` has no FK at all (W:284-291), so nothing to
support. Catalog FK check on a freshly loaded DDL returns zero violations.

### (c) event_outbox without FK to sessions
Intentional, verified, and consistent. W:295 comment states "no FK: deletion
events survive". Empirical: with two outbox rows (one carrying a session id,
one NULL), deleting the session leaves both rows; the session-bearing row
persists with a dangling `session_id`. This is required, not a bug: an
invalidation stream must publish the deletion event for a session whose
messages were just cascaded away. `event_session_idx` (W:300) is a partial
covering index for filtered replay (`WHERE session_id IS NOT NULL`), a lookup
aid, not an integrity constraint; dangling ids are semantically fine because
consumers resync by cursor (STO:184-188: cursor below floor, above head or
old epoch triggers RESYNC; STO:188-189 filtered scans return watermarks).
Consistency cross-check: STORAGE.md's replay query (STO:135) filters by
session_id and seq exactly matching the index; the keyset plan test PY:241
confirms index use without a temp sort. Residual writer obligations: nothing
validates that a 16-byte `session_id` value ever belonged to a real session
(W:295 length CHECK only), and `created_at_us` ordering is advisory - both
accepted because the outbox is explicitly noncanonical (STO:172-175).

### (d) workspace_state cursor CHECK
CONFIRMED at W:19: `0 <= event_floor_seq AND event_floor_seq <= event_head_seq`.
Probes: setting floor 1 with head 0 rejected; floor -1 rejected; head -5
rejected. The singleton row is pinned by `id INTEGER PRIMARY KEY CHECK(id=1)`
(W:10). Two weaknesses beyond the CHECK: head can be lowered (regression
passes whenever floor stays below the new head - probe confirmed), and
`cursor_epoch` has no immutability trigger (UPDATE accepted). Neither is a
CHECK violation; both are writer-owned. Also note `event_head_seq` and
`event_floor_seq` have no upper bound, which is fine, and no DEFAULT mismatch:
both default 0 (W:16-17).

### (e) payload_ready trigger (W:43-46) versus STRICT
Compose correctly; no defect. The BEFORE INSERT trigger observes STRICT-typed
NEW values, so `blobs.raw_bytes = NEW.raw_bytes` (W:45) compares INTEGER to
INTEGER and the `EXISTS` probe on `blobs.pk = NEW.blob_pk` cannot be fooled by
type coercion. Probes on the exact DDL confirm: TEXT inline payloads are
rejected by STRICT type enforcement before any CHECK or trigger semantics
matter, and a payload referencing an unknown blob pk or a mismatched
`raw_bytes` is rejected by the trigger (PY:84-87, RS:43-46). Note the trigger
intentionally validates registry state plus length only; codec-decoded length
and digest remain runtime concerns (see row 14).

### (f) approvals state machine
The specific question: can pending expire without broker? At the DDL level,
YES. Probes on the exact DDL: `approvals` has zero triggers (verified by
sqlite_schema query: empty list); `UPDATE approvals SET state=3` on a pending
row commits with `resolved_at_us` still NULL; a consumed row can be moved
back to pending (state 4 to 0 accepted); `mandatory_human` can be flipped
after creation (1 to 0 accepted, which then satisfies the W:239 CHECK without
any human identity). The only SQL constraint on the machine is W:239:
`state NOT IN (1,4) OR mandatory_human=0 OR human_client_id IS NOT NULL`
(terminal allowed/consumed states require a human client id when the grant
was mandatory). There is no `resolved_at_us`-vs-state CHECK comparable to the
ones executions (W:160) and tool_calls (W:208) have for `finished_at_us`, and
no expiry linkage (`expires_at_us > created_at_us` exists at W:238 but nothing
prevents resolving after expiry, since expiry evaluation is time-based and
runtime). Assessment: this is a deliberate boundary, announced twice in the
sources - W:222 ("Decision records are not capabilities: the trusted broker
still authorizes use") and STO:91-95 ("An approval row is evidence, not
authority... The writer must enforce allowed state transitions"). Rating:
PARTIAL - the human-identity invariant is real SQL, but the transition and
resolution bookkeeping the row implies is unowned in the DDL and weaker than
the sibling tables' patterns. Tightening candidate (requires a proposal, not
a silent edit): add
`CHECK((state=0 AND resolved_at_us IS NULL) OR (state<>0 AND resolved_at_us IS NOT NULL))`
to mirror W:160/W:208, plus an `mandatory_human` immutability trigger.

### (g) compaction_checkpoints FK to messages(session_pk, seq)
CONFIRMED sound. The FK at W:271-272 is
`FOREIGN KEY(session_pk, boundary_message_seq) REFERENCES messages(session_pk, seq)
ON DELETE NO ACTION DEFERRABLE INITIALLY DEFERRED`. The parent-key requirement
is satisfied by `UNIQUE(session_pk, seq)` at W:93 (verified present; SQLite
accepts the FK, which it would not at DDL load time otherwise). Empirical on
the exact DDL: inserting a checkpoint whose (session_pk, seq) matches a real
message commits; inserting one at seq 7 when only seq 5 exists is accepted at
statement time (deferred) and rejected at COMMIT with
`FOREIGN KEY constraint failed`. Session deletion is consistent: checkpoints
carry their own `session_pk ... ON DELETE CASCADE` (W:265), so the session
cascade removes checkpoints before the deferred boundary FK is evaluated;
PY:134's post-delete `foreign_key_check` returning empty supports this. The
deferral is the right choice for out-of-order writes inside one transaction
and matches STO:88-89 ("Deleting an individual promoted message requires
explicitly reconciling its receipt" - same NO ACTION discipline).

### (h) missing indexes for payload_roots scans
No missing index found. The GC anti-join
`SELECT pk FROM payloads WHERE pk NOT IN (SELECT payload_pk FROM payload_roots)`
plans as: SCAN payloads, then each of the eleven UNION ALL arms through its
covering index - `message_parts_payload_idx` (W:109),
`input_parts_payload_idx` (W:142), `executions_config_idx` (W:166),
`tools_input_idx` (W:213), `tools_output_idx` (W:214),
`tools_error_idx` (W:215), `context_baseline_idx` (W:261),
`context_snapshot_idx` (W:262), `compaction_summary_idx` (W:274),
`compaction_recent_idx` (W:275), `retained_payload_idx` (W:283). FK-support
probes all resolve: blobs delete checks payloads via `payloads_blob_idx`
(partial, usable because `blob_pk=?` implies NOT NULL); approvals tool FK via
`approvals_tool_idx`; inputs promoted FK via `inputs_message_idx`; execution
delete discovers tools/attempts/children through `tools_execution_idx`,
the attempts UNIQUE and `executions_parent_idx`. Performance caveat worth
recording: the `NOT IN (view)` form materializes the entire compound root
list into an ephemeral structure first (plan shows `LIST SUBQUERY 1` over
`COMPOUND QUERY`), so each GC pass pays O(total roots) build cost and memory
before scanning payloads. A per-arm `NOT EXISTS` formulation uses correlated
covering-index seeks per payloads row without materialization (probe
confirmed). Recommendation for the writer: use per-arm NOT EXISTS (or a
temp root table with ANALYZE) for large workspaces; the view itself is fine
for the coverage test PY:193-197, which is a correctness check, not a
performance contract.

## Part 4: additional adversarial findings (not on the checklist)

1. Tool terminal outputs mutable: `output_payload_pk` and `error_payload_pk`
   are not in the W:219 immutable list and empirically remain mutable after
   state=2 (success). Evidence swaps are possible by a buggy writer. Design
   assigns terminal-state immutability to the writer (STO:94-95); flag as a
   writer test requirement, not a DDL defect.
2. `sessions.next_message_seq` and `next_input_seq` can be lowered freely
   (probe accepted); seq allocation integrity is entirely writer-owned.
   `UNIQUE(session_pk, seq)` (W:93) catches collisions but nothing prevents
   head-regression style gaps.
3. `executions` cycle injection: impossible at DDL level. The immediate
   parent FK (W:148) forces parents to exist first, `CHECK(parent <> pk)`
   (W:159) blocks self-reference, and `execution_parent_immutable` (W:167-169)
   blocks repointing a non-NULL parent, so a cycle cannot be built via insert
   order or update (verified by trigger error on repoint; PY:151-154 tests
   the update path).
4. `schema_migrations` tamper: rows can be UPDATEd or DELETEd by anyone with
   SQL access (no trigger), but `open_existing` recomputes the checksum from
   the compile-time embedded DDL (SCHEMA_INIT:111-136) and fails closed on
   mismatch or missing row. Detection-at-open is sufficient for the
   new-file-only wave; PY/RS suites plus WRITER_RED:128-143 cover it.
5. `PRAGMA user_version` and `application_id` are transactional in SQLite
   (probe: values set inside a transaction disappear on ROLLBACK and persist
   on COMMIT). The initializer comment at SCHEMA_INIT:76-79 ("may or may not
   persist inside a transaction") is inaccurate; behavior is safe because
   the in-transaction set succeeds and the post-commit fallback
   (SCHEMA_INIT:83-85) is dead code, but the comment should be corrected when
   the file is next touched. If the fallback ever ran it would open a window
   with committed schema and user_version=0.
6. Catalog DDL (C:2-50): six STRICT tables, no conversation or secret-value
   columns (verified by PY:259-264), all WITHOUT ROWID FK lookups indexed
   (C:42, C:48-50). No defect found. `workspaces.status BETWEEN 0 AND 3`
   (C:17) has no transition constraints, consistent with the saga being
   runtime-owned (CC:203-208).

## Part 5: independent test runs (mandated commands)

Environment: cargo 1.96.0, rustc 1.96.0, Python 3.14.4, Python SQLite 3.46.1,
bundled Rust SQLite 3.50.2 (libsqlite3-sys 0.35.0, sqlite3.h:149).

1. `python3 -m unittest discover -s tests/bootstrap`
   at 2026-09-13T11:40:48Z: `Ran 68 tests in 0.158s`, `FAILED (errors=1)`.
   The single error is `tests.bootstrap.test_freeze_sources.
   FreezeSourceTests.test_invalid_commit_tree_or_license_pin_is_rejected`:
   `ValueError: Missing license SPDX identifier for x` raised from
   `tools/source_lock.py:76` because the base spec at
   `tests/bootstrap/test_freeze_sources.py:44-53` omits `licenseSpdx`. This is
   pre-existing and unrelated to storage: `tools/source_lock.py` is unmodified
   in the working tree (git status shows no tools/ change), and the error
   reproduces identically with the only modified tracked file
   (`crates/storage/src/lib.rs`, 2-line export diff) stashed. The storage
   contracts are not implicated.
2. Storage subset `python3 -m unittest tests.bootstrap.test_storage_schema_v2`
   (same session): `Ran 37 tests`, `OK`, zero failures, including the
   process-exit crash test (PY:281-305) and the mutation evidence recorded in
   `validation/storage-v2-mutation.log` (removing `payload_ready` fails
   `test_blob_gc_rejects_reattachment_to_tombstone`).
3. `cargo test -p opencode-rk-storage` (default target): FAILS TO COMPILE.
   `error[E0432]: unresolved import opencode_rk_storage::writer_v2` at
   `crates/storage/tests/writer_v2.rs:7`, plus an unused-mut warning at
   WRITER_RED:129. This is the expected RED state: the writer module the
   frozen suite imports does not exist yet (see Part 6). Exact output tail:
   `error: could not compile opencode-rk-storage (test "writer_v2") due to 1
   previous error; 1 warning emitted`.
4. `cargo test -p opencode-rk-storage --test schema_v2`
   at 2026-09-13T12:08:34Z: `test result: ok. 6 passed; 0 failed; 0 ignored`
   (`both_new_file_schemas_load_without_foreign_key_errors`,
   `payload_bytes_are_bounded_and_lengths_are_exact`,
   `deleting_blob_cannot_gain_references_or_be_resurrected`,
   `referenced_blob_cannot_be_claimed_by_gc`,
   `immutable_payloads_cannot_change_after_publication`,
   `state_outbox_and_cursor_roll_back_together`).

Interpretation: the DDL contract suites pass independently on both engines
available locally (Python 3.46.1, Rust-bundled 3.50.2). Neither is production
qualification (STO:163-168). The default cargo target failing at compile is
the correct TDD posture, not a regression, but it means no full-workspace
cargo regression has run in this wave.

## Part 6: evidence integrity observations

1. `validation/storage-v2-verification.md:12` records the six Rust tests as
   "not compiled or executed". In the current tree they DO compile and pass
   (Part 5, run 4). The receipt described the earlier wave state; it is now
   stale on that point. Not false for its timestamp, but any acceptance use
   must re-run.
2. `worklog/REPAIR-001.md:48` claims `python3 -m unittest discover -s
   tests/bootstrap -> Ran 31 tests, OK`, and REPAIR-001.md:32-35 claims the
   SPDX check in `tools/source_lock.py` was made conditional. The committed
   `tools/source_lock.py:75-76` has an UNconditional check, and the suite now
   runs 68 tests with the 1 error described in Part 5. The repair state
   described in that worklog does not match this tree (either never committed
   or reverted); its verification receipt does not reproduce.
3. `docs/storage/REVIEW.md:3-5` self-discloses as a one-author review. This
   document is the independent counterpart requested by that disclosure.
   REVIEW.md:43-51 ("Not certified by this wave") remains accurate: no frozen
   independent TDD acceptance, no power-loss proof, no benchmarks have been
   added by the implementation wave so far.

## Part 7: activation blockers and required writer obligations

Blockers (release gates that remain closed):
- Engine gate (row 16): bundled 3.50.2 < 3.50.7 backport floor. Activation of
  format-2 on the current bundle would violate the repository's own gate.
- Writer (`writer_v2.rs` module) and catalog initializer (`catalog_v2.rs`) do
  not exist; the frozen RED suite blocks compilation of the default test
  target until the writer lands.
- No power-loss/fsync/no-replace/OS-lock fault tests exist (CC:233-237
  remains a required-test list, not a test suite).

Writer obligations surfaced by this review (SQL deliberately does not own
them; the writer suite must):
- Apply the full connection contract (including `foreign_keys=ON`) to the
  connection returned by `initialize_workspace` - currently only
  `open_existing` configures (SCHEMA_INIT:138 vs SCHEMA_INIT:56). The frozen
  test WRITER_RED:53-71 already asserts this, so the RED suite covers the gap.
- Approval state transitions, resolution bookkeeping, mandatory_human
  immutability (Part 3f).
- Head monotonicity, cursor_epoch rotation protocol, prefix-only outbox
  pruning (Part 3d, W:301 comment).
- Blob quarantine (0 to 2) and unquarantine-with-revalidation (Part 3a).
- Terminal-state immutability for tool outputs and errors (Part 4.1).
- Expired-retry rejection before receipt lookup (STO:180-182).

Verdict summary: 8 CONFIRMED, 6 PARTIAL, 1 MISSING, 1 CONFIRMED-as-blocker
across the 16 rows; no incorrect invariant claim was found in REVIEW.md, but
six rows advertise more than the DDL owns, and one (quotas) has no owner at
all yet. The directed defect hunt found no broken invariant in the DDL; the
material findings are the approvals asymmetry (3f), the GC NOT IN
materialization cost (3h), and the mutable post-terminal tool outputs (4.1),
all of which are writer-scope by design and must become writer tests.
