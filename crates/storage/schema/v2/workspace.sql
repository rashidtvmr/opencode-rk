-- Storage format 2: NEW EMPTY DATABASE ONLY, not an in-place v1 migration.
-- Execute this complete DDL in one explicit transaction. Connection PRAGMAs live
-- in the initialization protocol; user_version is advanced only on commit.
CREATE TABLE schema_migrations (
    version INTEGER PRIMARY KEY CHECK(version > 0),
    checksum BLOB NOT NULL CHECK(length(checksum) = 32),
    applied_at_us INTEGER NOT NULL
) STRICT;
CREATE TABLE workspace_state (
    id INTEGER PRIMARY KEY CHECK(id = 1),
    workspace_id BLOB NOT NULL CHECK(length(workspace_id) = 16),
    cursor_epoch BLOB NOT NULL CHECK(length(cursor_epoch) = 16),
    format_version INTEGER NOT NULL CHECK(format_version = 2),
    owner_generation INTEGER NOT NULL DEFAULT 0 CHECK(owner_generation >= 0),
    clean_shutdown INTEGER NOT NULL DEFAULT 0 CHECK(clean_shutdown IN (0,1)),
    event_head_seq INTEGER NOT NULL DEFAULT 0,
    event_floor_seq INTEGER NOT NULL DEFAULT 0,
    created_at_us INTEGER NOT NULL,
    CHECK(0 <= event_floor_seq AND event_floor_seq <= event_head_seq)
) STRICT;
CREATE TABLE blobs (
    pk INTEGER PRIMARY KEY,
    hash BLOB NOT NULL UNIQUE CHECK(length(hash) = 32),
    state INTEGER NOT NULL DEFAULT 0 CHECK(state IN (0,1,2)), -- ready, deleting, unavailable
    codec INTEGER NOT NULL CHECK(codec IN (0,1)), -- raw, zstd
    raw_bytes INTEGER NOT NULL CHECK(raw_bytes >= 0),
    stored_bytes INTEGER NOT NULL CHECK(stored_bytes >= 52), -- includes header
    created_at_us INTEGER NOT NULL,
    verified_at_us INTEGER
) STRICT;
CREATE INDEX blobs_gc_idx ON blobs(state, created_at_us, pk);
CREATE TABLE payloads (
    pk INTEGER PRIMARY KEY,
    inline_data BLOB,
    blob_pk INTEGER REFERENCES blobs(pk) ON DELETE RESTRICT,
    raw_bytes INTEGER NOT NULL CHECK(raw_bytes >= 0),
    created_at_us INTEGER NOT NULL,
    CHECK((inline_data IS NOT NULL) + (blob_pk IS NOT NULL) = 1),
    CHECK(inline_data IS NULL OR
          (length(inline_data) <= 8192 AND raw_bytes = length(inline_data)))
) STRICT;
CREATE INDEX payloads_blob_idx ON payloads(blob_pk) WHERE blob_pk IS NOT NULL;
CREATE TRIGGER payload_ready BEFORE INSERT ON payloads
WHEN NEW.blob_pk IS NOT NULL AND NOT EXISTS (
    SELECT 1 FROM blobs WHERE pk = NEW.blob_pk AND state = 0 AND raw_bytes = NEW.raw_bytes
) BEGIN SELECT RAISE(ABORT, 'blob unavailable or length mismatch'); END;
CREATE TRIGGER payload_immutable BEFORE UPDATE ON payloads
BEGIN SELECT RAISE(ABORT, 'payloads are immutable'); END;
CREATE TRIGGER blob_gc_claim BEFORE UPDATE OF state ON blobs
WHEN NEW.state = 1 AND EXISTS (SELECT 1 FROM payloads WHERE blob_pk = OLD.pk)
BEGIN SELECT RAISE(ABORT, 'referenced blob cannot be collected'); END;
CREATE TRIGGER blob_no_resurrection BEFORE UPDATE OF state ON blobs
WHEN OLD.state = 1 AND NEW.state <> 1
BEGIN SELECT RAISE(ABORT, 'finish deletion before republishing'); END;
CREATE TRIGGER blob_identity_immutable BEFORE UPDATE OF hash, codec, raw_bytes, stored_bytes ON blobs
BEGIN SELECT RAISE(ABORT, 'blob identity is immutable'); END;

CREATE TABLE sessions (
    pk INTEGER PRIMARY KEY,
    id BLOB NOT NULL UNIQUE CHECK(length(id) = 16),
    title TEXT NOT NULL CHECK(length(CAST(title AS BLOB)) <= 1024),
    state INTEGER NOT NULL DEFAULT 0 CHECK(state IN (0,1)), -- active, archived
    agent_name TEXT NOT NULL DEFAULT 'default' CHECK(length(CAST(agent_name AS BLOB)) BETWEEN 1 AND 256),
    provider_id TEXT CHECK(provider_id IS NULL OR length(CAST(provider_id AS BLOB)) BETWEEN 1 AND 128),
    model_id TEXT CHECK(model_id IS NULL OR length(CAST(model_id AS BLOB)) BETWEEN 1 AND 512),
    effort_json TEXT CHECK(effort_json IS NULL OR
        (length(CAST(effort_json AS BLOB)) <= 1024 AND json_valid(effort_json))),
    fork_parent_id BLOB CHECK(fork_parent_id IS NULL OR length(fork_parent_id) = 16),
    fork_message_seq INTEGER,
    next_message_seq INTEGER NOT NULL DEFAULT 1 CHECK(next_message_seq > 0),
    next_input_seq INTEGER NOT NULL DEFAULT 1 CHECK(next_input_seq > 0),
    created_at_us INTEGER NOT NULL,
    updated_at_us INTEGER NOT NULL,
    archived_at_us INTEGER,
    CHECK((provider_id IS NULL) = (model_id IS NULL)),
    CHECK((fork_parent_id IS NULL AND fork_message_seq IS NULL) OR
          (fork_parent_id IS NOT NULL AND fork_message_seq IS NOT NULL AND fork_message_seq >= 0)),
    CHECK((state = 0 AND archived_at_us IS NULL) OR (state = 1 AND archived_at_us IS NOT NULL))
) STRICT;
CREATE INDEX sessions_state_updated_idx ON sessions(state, updated_at_us DESC, pk DESC);
CREATE INDEX sessions_updated_idx ON sessions(updated_at_us DESC, pk DESC);
CREATE TABLE messages (
    pk INTEGER PRIMARY KEY,
    id BLOB NOT NULL UNIQUE CHECK(length(id) = 16),
    session_pk INTEGER NOT NULL REFERENCES sessions(pk) ON DELETE CASCADE,
    seq INTEGER NOT NULL CHECK(seq > 0),
    role INTEGER NOT NULL CHECK(role BETWEEN 0 AND 3), -- system, user, assistant, tool
    status INTEGER NOT NULL CHECK(status BETWEEN 0 AND 3), -- open, complete, failed, interrupted
    provider_id TEXT CHECK(provider_id IS NULL OR length(CAST(provider_id AS BLOB)) BETWEEN 1 AND 128),
    model_id TEXT CHECK(model_id IS NULL OR length(CAST(model_id AS BLOB)) BETWEEN 1 AND 512),
    created_at_us INTEGER NOT NULL,
    completed_at_us INTEGER,
    UNIQUE(session_pk, seq),
    UNIQUE(pk, session_pk), -- composite FK target for same-session ownership
    CHECK((provider_id IS NULL) = (model_id IS NULL)),
    CHECK((status = 0 AND completed_at_us IS NULL) OR (status <> 0 AND completed_at_us IS NOT NULL))
) STRICT;
CREATE TABLE message_parts (
    message_pk INTEGER NOT NULL REFERENCES messages(pk) ON DELETE CASCADE,
    ordinal INTEGER NOT NULL CHECK(ordinal BETWEEN 0 AND 255),
    kind INTEGER NOT NULL CHECK(kind BETWEEN 0 AND 15),
    payload_pk INTEGER NOT NULL REFERENCES payloads(pk) ON DELETE RESTRICT,
    mime TEXT CHECK(mime IS NULL OR length(CAST(mime AS BLOB)) <= 255),
    name TEXT CHECK(name IS NULL OR length(CAST(name AS BLOB)) <= 1024),
    metadata_json TEXT CHECK(metadata_json IS NULL OR
        (length(CAST(metadata_json AS BLOB)) <= 4096 AND json_valid(metadata_json))),
    PRIMARY KEY(message_pk, ordinal)
) STRICT, WITHOUT ROWID;
CREATE INDEX message_parts_payload_idx ON message_parts(payload_pk);
CREATE TRIGGER message_identity_immutable BEFORE UPDATE OF id, session_pk, seq, role ON messages
BEGIN SELECT RAISE(ABORT, 'message identity is immutable'); END;
CREATE TABLE session_inputs (
    pk INTEGER PRIMARY KEY,
    id BLOB NOT NULL UNIQUE CHECK(length(id) = 16),
    session_pk INTEGER NOT NULL REFERENCES sessions(pk) ON DELETE CASCADE,
    seq INTEGER NOT NULL CHECK(seq > 0),
    delivery INTEGER NOT NULL CHECK(delivery IN (0,1)), -- steer, queue
    state INTEGER NOT NULL DEFAULT 0 CHECK(state IN (0,1,2)), -- pending, promoted, cancelled
    request_hash BLOB NOT NULL CHECK(length(request_hash) = 32),
    promoted_message_pk INTEGER,
    created_at_us INTEGER NOT NULL,
    promoted_at_us INTEGER,
    UNIQUE(session_pk, seq),
    FOREIGN KEY(promoted_message_pk, session_pk) REFERENCES messages(pk, session_pk)
        ON DELETE NO ACTION DEFERRABLE INITIALLY DEFERRED,
    CHECK((state = 1 AND promoted_message_pk IS NOT NULL AND promoted_at_us IS NOT NULL) OR
          (state <> 1 AND promoted_message_pk IS NULL AND promoted_at_us IS NULL))
) STRICT;
CREATE INDEX inputs_pending_idx ON session_inputs(session_pk, delivery, seq) WHERE state = 0;
CREATE INDEX inputs_message_idx ON session_inputs(promoted_message_pk, session_pk) WHERE promoted_message_pk IS NOT NULL;
CREATE TRIGGER input_identity_immutable BEFORE UPDATE OF id, session_pk, seq, delivery, request_hash ON session_inputs
BEGIN SELECT RAISE(ABORT, 'admission identity is immutable'); END;
CREATE TABLE session_input_parts (
    input_pk INTEGER NOT NULL REFERENCES session_inputs(pk) ON DELETE CASCADE,
    ordinal INTEGER NOT NULL CHECK(ordinal BETWEEN 0 AND 255),
    kind INTEGER NOT NULL CHECK(kind BETWEEN 0 AND 15),
    payload_pk INTEGER NOT NULL REFERENCES payloads(pk) ON DELETE RESTRICT,
    mime TEXT CHECK(mime IS NULL OR length(CAST(mime AS BLOB)) <= 255),
    name TEXT CHECK(name IS NULL OR length(CAST(name AS BLOB)) <= 1024),
    PRIMARY KEY(input_pk, ordinal)
) STRICT, WITHOUT ROWID;
CREATE INDEX input_parts_payload_idx ON session_input_parts(payload_pk);

CREATE TABLE executions (
    pk INTEGER PRIMARY KEY,
    id BLOB NOT NULL UNIQUE CHECK(length(id) = 16),
    session_pk INTEGER NOT NULL REFERENCES sessions(pk) ON DELETE CASCADE,
    parent_execution_pk INTEGER REFERENCES executions(pk) ON DELETE SET NULL,
    mode INTEGER NOT NULL CHECK(mode IN (0,1)), -- foreground, background
    state INTEGER NOT NULL CHECK(state BETWEEN 0 AND 5), -- queued, running, complete, failed, uncertain, cancelled
    owner_generation INTEGER NOT NULL CHECK(owner_generation >= 0),
    config_payload_pk INTEGER NOT NULL REFERENCES payloads(pk) ON DELETE RESTRICT,
    provider_id TEXT NOT NULL CHECK(length(CAST(provider_id AS BLOB)) BETWEEN 1 AND 128),
    model_id TEXT NOT NULL CHECK(length(CAST(model_id AS BLOB)) BETWEEN 1 AND 512),
    created_at_us INTEGER NOT NULL,
    started_at_us INTEGER,
    finished_at_us INTEGER,
    UNIQUE(pk, session_pk),
    CHECK(parent_execution_pk IS NULL OR parent_execution_pk <> pk),
    CHECK((state IN (0,1,4) AND finished_at_us IS NULL) OR (state IN (2,3,5) AND finished_at_us IS NOT NULL))
) STRICT;
CREATE UNIQUE INDEX executions_single_owner_idx ON executions(session_pk) WHERE state IN (0,1,4);
CREATE INDEX executions_session_idx ON executions(session_pk, pk DESC);
CREATE INDEX executions_parent_idx ON executions(parent_execution_pk, pk) WHERE parent_execution_pk IS NOT NULL;
CREATE INDEX executions_recovery_idx ON executions(owner_generation, pk) WHERE state IN (0,1,4);
CREATE INDEX executions_config_idx ON executions(config_payload_pk);
CREATE TRIGGER execution_parent_immutable BEFORE UPDATE OF parent_execution_pk ON executions
WHEN NEW.parent_execution_pk IS NOT NULL AND NEW.parent_execution_pk IS NOT OLD.parent_execution_pk
BEGIN SELECT RAISE(ABORT, 'execution parent is immutable'); END;
CREATE TABLE provider_attempts (
    pk INTEGER PRIMARY KEY,
    id BLOB NOT NULL UNIQUE CHECK(length(id) = 16),
    execution_pk INTEGER NOT NULL REFERENCES executions(pk) ON DELETE CASCADE,
    ordinal INTEGER NOT NULL CHECK(ordinal >= 0),
    state INTEGER NOT NULL CHECK(state BETWEEN 0 AND 5), -- prepared, dispatched, success, failure, uncertain, abandoned
    request_hash BLOB NOT NULL CHECK(length(request_hash) = 32),
    through_message_seq INTEGER NOT NULL CHECK(through_message_seq >= 0),
    provider_request_id TEXT CHECK(provider_request_id IS NULL OR length(CAST(provider_request_id AS BLOB)) <= 512),
    created_at_us INTEGER NOT NULL,
    dispatched_at_us INTEGER,
    finished_at_us INTEGER,
    UNIQUE(execution_pk, ordinal),
    CHECK((state IN (2,3,5) AND finished_at_us IS NOT NULL) OR (state IN (0,1,4) AND finished_at_us IS NULL))
) STRICT;
CREATE INDEX attempts_recovery_idx ON provider_attempts(state, pk) WHERE state IN (0,1,4);
CREATE TABLE tool_calls (
    pk INTEGER PRIMARY KEY,
    id BLOB NOT NULL UNIQUE CHECK(length(id) = 16),
    session_pk INTEGER NOT NULL,
    execution_pk INTEGER NOT NULL,
    assistant_message_pk INTEGER NOT NULL,
    ordinal INTEGER NOT NULL CHECK(ordinal BETWEEN 0 AND 255),
    provider_call_id TEXT CHECK(provider_call_id IS NULL OR length(CAST(provider_call_id AS BLOB)) <= 512),
    name TEXT NOT NULL CHECK(length(CAST(name AS BLOB)) BETWEEN 1 AND 256),
    state INTEGER NOT NULL CHECK(state BETWEEN 0 AND 5), -- planned, dispatched, success, failure, cancelled, uncertain
    intent_hash BLOB NOT NULL CHECK(length(intent_hash) = 32),
    input_payload_pk INTEGER NOT NULL REFERENCES payloads(pk) ON DELETE RESTRICT,
    output_payload_pk INTEGER REFERENCES payloads(pk) ON DELETE RESTRICT,
    error_payload_pk INTEGER REFERENCES payloads(pk) ON DELETE RESTRICT,
    created_at_us INTEGER NOT NULL,
    dispatched_at_us INTEGER,
    finished_at_us INTEGER,
    UNIQUE(assistant_message_pk, ordinal),
    UNIQUE(pk, session_pk),
    FOREIGN KEY(execution_pk, session_pk) REFERENCES executions(pk, session_pk) ON DELETE CASCADE,
    FOREIGN KEY(assistant_message_pk, session_pk) REFERENCES messages(pk, session_pk) ON DELETE CASCADE,
    CHECK(state <> 2 OR output_payload_pk IS NOT NULL),
    CHECK((state IN (2,3,4) AND finished_at_us IS NOT NULL) OR (state IN (0,1,5) AND finished_at_us IS NULL))
) STRICT;
CREATE UNIQUE INDEX tool_provider_call_idx ON tool_calls(assistant_message_pk, provider_call_id) WHERE provider_call_id IS NOT NULL;
CREATE INDEX tools_execution_idx ON tool_calls(execution_pk, session_pk);
CREATE INDEX tools_recovery_idx ON tool_calls(state, pk) WHERE state IN (0,1,5);
CREATE INDEX tools_input_idx ON tool_calls(input_payload_pk);
CREATE INDEX tools_output_idx ON tool_calls(output_payload_pk) WHERE output_payload_pk IS NOT NULL;
CREATE INDEX tools_error_idx ON tool_calls(error_payload_pk) WHERE error_payload_pk IS NOT NULL;
CREATE TRIGGER tool_assistant_owner BEFORE INSERT ON tool_calls
WHEN NOT EXISTS (SELECT 1 FROM messages WHERE pk = NEW.assistant_message_pk AND role = 2 AND session_pk = NEW.session_pk)
BEGIN SELECT RAISE(ABORT, 'tool owner must be an assistant in the same session'); END;
CREATE TRIGGER tool_binding_immutable BEFORE UPDATE OF id, session_pk, execution_pk, assistant_message_pk, ordinal, provider_call_id, name, intent_hash, input_payload_pk ON tool_calls
BEGIN SELECT RAISE(ABORT, 'tool invocation binding is immutable'); END;

-- Decision records are not capabilities: the trusted broker still authorizes use.
CREATE TABLE approvals (
    pk INTEGER PRIMARY KEY,
    id BLOB NOT NULL UNIQUE CHECK(length(id) = 16),
    session_pk INTEGER NOT NULL REFERENCES sessions(pk) ON DELETE CASCADE,
    tool_call_pk INTEGER,
    intent_hash BLOB NOT NULL CHECK(length(intent_hash) = 32),
    policy_generation INTEGER NOT NULL CHECK(policy_generation >= 0),
    action TEXT NOT NULL CHECK(length(CAST(action AS BLOB)) BETWEEN 1 AND 128),
    state INTEGER NOT NULL CHECK(state BETWEEN 0 AND 4), -- pending, allowed-once, denied, expired, consumed
    mandatory_human INTEGER NOT NULL CHECK(mandatory_human IN (0,1)),
    human_client_id BLOB CHECK(human_client_id IS NULL OR length(human_client_id) = 16),
    created_at_us INTEGER NOT NULL,
    expires_at_us INTEGER NOT NULL,
    resolved_at_us INTEGER,
    FOREIGN KEY(tool_call_pk, session_pk) REFERENCES tool_calls(pk, session_pk) ON DELETE CASCADE,
    CHECK(expires_at_us > created_at_us),
    CHECK(state NOT IN (1,4) OR mandatory_human = 0 OR human_client_id IS NOT NULL)
) STRICT;
CREATE INDEX approvals_session_idx ON approvals(session_pk, pk);
CREATE INDEX approvals_pending_idx ON approvals(session_pk, created_at_us, pk) WHERE state = 0;
CREATE INDEX approvals_tool_idx ON approvals(tool_call_pk, session_pk) WHERE tool_call_pk IS NOT NULL;
CREATE TABLE approval_resources (
    approval_pk INTEGER NOT NULL REFERENCES approvals(pk) ON DELETE CASCADE,
    ordinal INTEGER NOT NULL CHECK(ordinal BETWEEN 0 AND 255),
    resource TEXT NOT NULL CHECK(length(CAST(resource AS BLOB)) BETWEEN 1 AND 4096),
    PRIMARY KEY(approval_pk, ordinal)
) STRICT, WITHOUT ROWID;
CREATE TABLE context_epochs (
    pk INTEGER PRIMARY KEY,
    session_pk INTEGER NOT NULL REFERENCES sessions(pk) ON DELETE CASCADE,
    epoch INTEGER NOT NULL CHECK(epoch > 0),
    baseline_payload_pk INTEGER NOT NULL REFERENCES payloads(pk) ON DELETE RESTRICT,
    snapshot_payload_pk INTEGER NOT NULL REFERENCES payloads(pk) ON DELETE RESTRICT,
    created_at_us INTEGER NOT NULL,
    closed_at_us INTEGER,
    UNIQUE(session_pk, epoch)
) STRICT;
CREATE UNIQUE INDEX context_open_idx ON context_epochs(session_pk) WHERE closed_at_us IS NULL;
CREATE INDEX context_baseline_idx ON context_epochs(baseline_payload_pk);
CREATE INDEX context_snapshot_idx ON context_epochs(snapshot_payload_pk);
CREATE TABLE compaction_checkpoints (
    pk INTEGER PRIMARY KEY,
    session_pk INTEGER NOT NULL REFERENCES sessions(pk) ON DELETE CASCADE,
    boundary_message_seq INTEGER NOT NULL CHECK(boundary_message_seq > 0),
    summary_payload_pk INTEGER NOT NULL REFERENCES payloads(pk) ON DELETE RESTRICT,
    recent_payload_pk INTEGER REFERENCES payloads(pk) ON DELETE RESTRICT,
    created_at_us INTEGER NOT NULL,
    UNIQUE(session_pk, boundary_message_seq),
    FOREIGN KEY(session_pk, boundary_message_seq) REFERENCES messages(session_pk, seq)
        ON DELETE NO ACTION DEFERRABLE INITIALLY DEFERRED
) STRICT;
CREATE INDEX compaction_summary_idx ON compaction_checkpoints(summary_payload_pk);
CREATE INDEX compaction_recent_idx ON compaction_checkpoints(recent_payload_pk) WHERE recent_payload_pk IS NOT NULL;
CREATE TABLE retained_payloads (
    owner_id BLOB NOT NULL CHECK(length(owner_id) = 16),
    purpose INTEGER NOT NULL CHECK(purpose IN (0,1,2)), -- artifact, share, backup pin
    payload_pk INTEGER NOT NULL REFERENCES payloads(pk) ON DELETE RESTRICT,
    created_at_us INTEGER NOT NULL,
    PRIMARY KEY(owner_id, purpose, payload_pk)
) STRICT, WITHOUT ROWID;
CREATE INDEX retained_payload_idx ON retained_payloads(payload_pk);
CREATE TABLE operation_receipts (
    operation_id BLOB PRIMARY KEY CHECK(length(operation_id) = 16),
    request_hash BLOB NOT NULL CHECK(length(request_hash) = 32),
    kind TEXT NOT NULL CHECK(length(CAST(kind AS BLOB)) BETWEEN 1 AND 128),
    result_json TEXT NOT NULL CHECK(length(CAST(result_json AS BLOB)) <= 4096 AND json_valid(result_json)),
    created_at_us INTEGER NOT NULL,
    retry_until_us INTEGER NOT NULL CHECK(retry_until_us > created_at_us)
) STRICT, WITHOUT ROWID;
CREATE INDEX receipts_expiry_idx ON operation_receipts(retry_until_us, operation_id);
CREATE TABLE event_outbox (
    seq INTEGER PRIMARY KEY CHECK(seq > 0), -- allocate from workspace_state in same transaction
    session_id BLOB CHECK(session_id IS NULL OR length(session_id) = 16), -- no FK: deletion events survive
    kind TEXT NOT NULL CHECK(length(CAST(kind AS BLOB)) BETWEEN 1 AND 128),
    payload_json TEXT NOT NULL CHECK(length(CAST(payload_json AS BLOB)) <= 4096 AND json_valid(payload_json)),
    created_at_us INTEGER NOT NULL
) STRICT;
CREATE INDEX event_session_idx ON event_outbox(session_id, seq) WHERE session_id IS NOT NULL;
-- Retention deletes only a sequence prefix. Do not punch holes with per-row TTL.
CREATE VIEW payload_roots AS
    SELECT payload_pk FROM message_parts UNION ALL
    SELECT payload_pk FROM session_input_parts UNION ALL
    SELECT config_payload_pk FROM executions UNION ALL
    SELECT input_payload_pk FROM tool_calls UNION ALL
    SELECT output_payload_pk FROM tool_calls WHERE output_payload_pk IS NOT NULL UNION ALL
    SELECT error_payload_pk FROM tool_calls WHERE error_payload_pk IS NOT NULL UNION ALL
    SELECT baseline_payload_pk FROM context_epochs UNION ALL
    SELECT snapshot_payload_pk FROM context_epochs UNION ALL
    SELECT summary_payload_pk FROM compaction_checkpoints UNION ALL
    SELECT recent_payload_pk FROM compaction_checkpoints WHERE recent_payload_pk IS NOT NULL UNION ALL
    SELECT payload_pk FROM retained_payloads;
