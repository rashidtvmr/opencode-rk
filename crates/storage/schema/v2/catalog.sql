-- opencode-rk format 2 installation catalog. NEW FILE ONLY; no conversation bodies.
CREATE TABLE schema_migrations (
    version INTEGER PRIMARY KEY CHECK(version > 0),
    checksum BLOB NOT NULL CHECK(length(checksum) = 32),
    applied_at_us INTEGER NOT NULL
) STRICT;
CREATE TABLE catalog_state (
    id INTEGER PRIMARY KEY CHECK(id = 1),
    installation_id BLOB NOT NULL CHECK(length(installation_id) = 16),
    format_version INTEGER NOT NULL CHECK(format_version = 2)
) STRICT;
CREATE TABLE workspaces (
    pk INTEGER PRIMARY KEY,
    id BLOB NOT NULL UNIQUE CHECK(length(id) = 16),
    label TEXT NOT NULL CHECK(length(CAST(label AS BLOB)) <= 1024),
    project_root TEXT CHECK(project_root IS NULL OR length(CAST(project_root AS BLOB)) <= 4096),
    status INTEGER NOT NULL CHECK(status BETWEEN 0 AND 3), -- provisioning, ready, deleting, unavailable
    created_at_us INTEGER NOT NULL,
    last_opened_at_us INTEGER,
    cached_session_count INTEGER NOT NULL DEFAULT 0 CHECK(cached_session_count >= 0)
) STRICT;
CREATE INDEX workspaces_recent_idx ON workspaces(last_opened_at_us DESC, pk DESC) WHERE status = 1;
CREATE TABLE app_settings (
    key TEXT PRIMARY KEY CHECK(length(CAST(key AS BLOB)) BETWEEN 1 AND 128),
    value_json TEXT NOT NULL CHECK(length(CAST(value_json AS BLOB)) <= 4096 AND json_valid(value_json)),
    revision INTEGER NOT NULL CHECK(revision > 0),
    updated_at_us INTEGER NOT NULL
) STRICT, WITHOUT ROWID;
CREATE TABLE provider_accounts (
    pk INTEGER PRIMARY KEY,
    id BLOB NOT NULL UNIQUE CHECK(length(id) = 16),
    provider_id TEXT NOT NULL CHECK(length(CAST(provider_id AS BLOB)) BETWEEN 1 AND 128),
    label TEXT NOT NULL CHECK(length(CAST(label AS BLOB)) <= 256),
    secret_ref TEXT NOT NULL CHECK(length(CAST(secret_ref AS BLOB)) BETWEEN 1 AND 1024),
    secret_generation INTEGER NOT NULL DEFAULT 1 CHECK(secret_generation > 0),
    enabled INTEGER NOT NULL DEFAULT 1 CHECK(enabled IN (0,1)),
    health INTEGER NOT NULL DEFAULT 0 CHECK(health BETWEEN 0 AND 3), -- unknown, ready, cooldown, reauth
    priority INTEGER NOT NULL DEFAULT 100 CHECK(priority BETWEEN 0 AND 1000000),
    last_used_at_us INTEGER,
    created_at_us INTEGER NOT NULL
) STRICT;
CREATE INDEX accounts_route_idx ON provider_accounts(provider_id, priority, pk) WHERE enabled = 1;
CREATE TABLE provider_account_model_locks (
    account_pk INTEGER NOT NULL REFERENCES provider_accounts(pk) ON DELETE CASCADE,
    model_id TEXT NOT NULL CHECK(length(CAST(model_id AS BLOB)) BETWEEN 1 AND 512), -- '*' is account-wide
    retry_at_us INTEGER NOT NULL,
    reason TEXT NOT NULL CHECK(length(CAST(reason AS BLOB)) <= 256),
    PRIMARY KEY(account_pk, model_id)
) STRICT, WITHOUT ROWID;
CREATE INDEX account_locks_expiry_idx ON provider_account_model_locks(retry_at_us, account_pk, model_id);
