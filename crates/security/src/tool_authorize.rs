//! Tool/executor authorization wiring (DISC-105, RC-AUD006-01).
//!
//! Pure argv-only entry point so the tool executor shell path and the server
//! tool path can call [`PermissionBroker::authorize`] before any spawn, with
//! no `bash -c` bypass: opaque shell strings map to [`ToolGate::HumanGate`]
//! (broker `RequireHuman`), never to direct execution. A denial runs zero
//! caller effects by construction — [`run_if_allowed`] executes the effect
//! closure only on `Allow`. Denials are recorded in a bounded, secret-redacted
//! audit trail ([`MAX_TOOL_AUDIT`]).
//!
//! Baseline-first semantics follow `super::decide` (lib.rs:291-339): mandatory
//! broker `Deny` is never lifted by grants or `*` permission scope; only
//! `RequireHuman` may be satisfied by a covering, fresh, version-current,
//! single-use grant. Policy version bumps invalidate cached approvals via
//! [`ExpectedScope::policy_version`] (stale → deny, ledger unburned).
//!
//! Pure: no spawn, no FS, no threads, no clock. The caller supplies `now`
//! inside [`ExpectedScope`]; execution itself stays in the caller's sandboxed
//! executor. `forbid(unsafe_code)`.
#![forbid(unsafe_code)]

use super::app_policy::{AppDecision, ExpectedScope, Grant, GrantLedger, OperationDigest};
use super::{Decision, FileAction, OperationIntent, PermissionBroker};
use std::path::PathBuf;

/// Bounded tool-authorizer audit (mirrors `MAX_AUDIT_ENTRIES` discipline).
pub const MAX_TOOL_AUDIT: usize = 256;
/// Max redacted audit-text bytes retained per entry (byte budget).
pub const MAX_AUDIT_TEXT_BYTES: usize = 512;

/// Gate outcome returned to the executor/server tool path.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ToolGate {
    /// Proceed to the caller's sandboxed spawn.
    Allow,
    /// Terminal deny: must not spawn, touch FS, or retry without a new grant.
    Deny { reason: String },
    /// Pause for an explicit human grant; resume once via [`authorize_with_grant`].
    HumanGate { reason: String },
}

/// One redacted audit entry. Intent text is secret-scrubbed at record time;
/// raw secrets never reach logs, history, or errors.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ToolAuditEntry {
    pub seq: u64,
    pub gate: ToolGateSummary,
    pub intent_text: String,
}

/// Redactable gate summary (no reason strings retained: they may echo input).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ToolGateSummary {
    Allow,
    Deny,
    HumanGate,
}

/// Intent constructor for the executor shell path. `bash -c <script>` (or any
/// `shell` tool with a `command` string) maps to an opaque `Process` intent:
/// the broker forces a human gate, so the direct-spawn bypass is closed.
#[must_use]
pub fn shell_intent(command: &str, cwd: &PathBuf) -> OperationIntent {
    OperationIntent::Process {
        program: "bash".to_owned(),
        args: vec!["-c".to_owned(), command.to_owned()],
        cwd: cwd.clone(),
    }
}

/// Intent constructor for direct-argv execution (preferred): no opaque string.
#[must_use]
pub fn argv_intent(program: &str, args: &[String], cwd: &PathBuf) -> OperationIntent {
    OperationIntent::Process {
        program: program.to_owned(),
        args: args.to_vec(),
        cwd: cwd.clone(),
    }
}

/// Intent constructor for the server tool path: one tool call = one intent.
/// File-backed tool arguments additionally bind a [`FileAction::Write`]
/// intent so project-root/destructive gates apply.
#[must_use]
pub fn server_tool_intent(name: &str, description: &str) -> OperationIntent {
    OperationIntent::Tool {
        name: name.to_owned(),
        description: description.to_owned(),
    }
}

/// Broker-backed authorizer for tool/executor/server paths. Owns nothing
/// global: the caller owns the broker, ledger, and expected scope.
pub struct ToolAuthorizer<'a> {
    broker: &'a PermissionBroker,
    ledger: GrantLedger,
    audit: Vec<ToolAuditEntry>,
    next_seq: u64,
}

impl<'a> ToolAuthorizer<'a> {
    #[must_use]
    pub fn new(broker: &'a PermissionBroker) -> Self {
        Self {
            broker,
            ledger: GrantLedger::new(),
            audit: Vec::new(),
            next_seq: 0,
        }
    }

    #[must_use]
    pub fn audit_len(&self) -> usize {
        self.audit.len()
    }

    #[must_use]
    pub fn audit(&self) -> &[ToolAuditEntry] {
        &self.audit
    }

    /// Authorize without a grant. `Allow` → caller may spawn; anything else →
    /// caller must not produce side effects.
    pub fn authorize(&mut self, intent: &OperationIntent) -> ToolGate {
        let gate = match self.broker.authorize(intent) {
            Decision::Allow => ToolGate::Allow,
            Decision::Deny { .. } => ToolGate::Deny {
                reason: String::new(),
            },
            Decision::RequireHuman { .. } => ToolGate::HumanGate {
                reason: String::new(),
            },
        };
        self.record(intent, &gate);
        gate
    }

    /// Authorize with an explicit human grant (single-use, version-bound).
    pub fn authorize_with_grant(
        &mut self,
        intent: &OperationIntent,
        grant: &Grant,
        expected: &ExpectedScope,
    ) -> ToolGate {
        let gate = match super::app_policy::decide(
            self.broker,
            intent,
            Some(grant),
            expected,
            &mut self.ledger,
        ) {
            AppDecision::Allow => ToolGate::Allow,
            AppDecision::Deny { .. } => ToolGate::Deny {
                reason: String::new(),
            },
            AppDecision::HumanOnly { .. } => ToolGate::HumanGate {
                reason: String::new(),
            },
        };
        self.record(intent, &gate);
        gate
    }

    fn record(&mut self, intent: &OperationIntent, gate: &ToolGate) {
        let summary = match gate {
            ToolGate::Allow => ToolGateSummary::Allow,
            ToolGate::Deny { .. } => ToolGateSummary::Deny,
            ToolGate::HumanGate { .. } => ToolGateSummary::HumanGate,
        };
        let mut text = redact_secrets(&intent_fingerprint_text(intent));
        if text.len() > MAX_AUDIT_TEXT_BYTES {
            text.truncate(MAX_AUDIT_TEXT_BYTES);
        }
        let seq = self.next_seq;
        self.next_seq = self.next_seq.saturating_add(1);
        self.audit.push(ToolAuditEntry {
            seq,
            gate: summary,
            intent_text: text,
        });
        if self.audit.len() > MAX_TOOL_AUDIT {
            let overflow = self.audit.len() - MAX_TOOL_AUDIT;
            self.audit.drain(..overflow);
        }
    }
}

/// Run `effect` only when `gate` is [`ToolGate::Allow`]. Denial and human-gate
/// outcomes never invoke the closure: no process, no file side effects.
pub fn run_if_allowed<T>(gate: &ToolGate, effect: impl FnOnce() -> T) -> Option<T> {
    match gate {
        ToolGate::Allow => Some(effect()),
        ToolGate::Deny { .. } | ToolGate::HumanGate { .. } => None,
    }
}

/// Stable binding text for one intent: kind tag plus content digest hex.
/// The digest changes when any byte (path, argv, SQL, tool input) changes, so
/// a grant cannot be retargeted — but raw intent bytes (which may carry
/// secrets) never enter logs, history, or errors.
fn intent_fingerprint_text(intent: &OperationIntent) -> String {
    let (kind, descriptor) = match intent {
        OperationIntent::File { action, path } => {
            ("file", format!("{action:?}:{}", path.to_string_lossy()))
        }
        OperationIntent::Process { program, args, .. } => {
            ("process", format!("{program}:{}argv", args.len()))
        }
        OperationIntent::Sql { database, .. } => ("sql", database.clone()),
        OperationIntent::Tool { name, .. } => ("tool", name.clone()),
    };
    let text = format!("{kind}:{descriptor}:{}", digest_of(intent));
    let mut redacted = redact_secrets(&text);
    if redacted.len() > MAX_AUDIT_TEXT_BYTES {
        redacted.truncate(MAX_AUDIT_TEXT_BYTES);
    }
    redacted
}

/// Scrub secret-bearing substrings from text destined for logs/history/errors.
///
/// Redaction rules (stdlib only, deterministic):
/// - `KEY=VALUE` / `KEY: VALUE` pairs whose key looks secret-bearing
///   (`api_key`, `secret`, `token`, `password`, `passwd`, `private_key`,
///   `credential`, `auth`, `access_key`, `session_key`, `client_secret`,
///   `bearer`) have their value replaced with `[REDACTED]`.
/// - Loose high-entropy-looking tokens (`sk-...`, `sk_live_...`,
///   `ghp_...`/`gho_...`, `xox...`, `AKIA...`, `Bearer <tok>`) are replaced.
/// - The value half of any `key=value` pair that still contains a
///   secret-looking marker after the above passes is scrubbed.
///
/// ponytail: pattern redaction, not an allowlist parser; upgrade to a typed
/// secret taxonomy when one is approved. Ceiling: crafted exfil strings can
/// dodge patterns — the broker deny (never-secret-readable) is the real
/// boundary; this keeps the audit trail from echoing the obvious cases.
#[must_use]
pub fn redact_secrets(text: &str) -> String {
    const MARKERS: [&str; 13] = [
        "API_KEY",
        "SECRET",
        "TOKEN",
        "PASSWORD",
        "PASSWD",
        "PRIVATE_KEY",
        "CREDENTIAL",
        "ACCESS_KEY",
        "SESSION_KEY",
        "CLIENT_SECRET",
        "BEARER",
        "AUTH",
        "AWS_",
    ];
    fn key_is_secret(key: &str) -> bool {
        let upper = key.to_ascii_uppercase();
        MARKERS.iter().any(|m| upper.contains(m))
    }
    fn token_is_secret(token: &str) -> bool {
        let t =
            token.trim_matches(|c: char| c == '\'' || c == '"' || c == ',' || c == ';' || c == ')');
        let lower = t.to_ascii_lowercase();
        lower.starts_with("sk-")
            || lower.starts_with("sk_live")
            || lower.starts_with("ghp_")
            || lower.starts_with("gho_")
            || lower.starts_with("xox")
            || t.starts_with("AKIA")
    }
    let mut parts: Vec<String> = Vec::new();
    let mut skip_next = false;
    for raw in text.split(' ') {
        if skip_next {
            skip_next = false;
            parts.push("[REDACTED]".to_owned());
            continue;
        }
        // `Bearer <token>`: drop the following token's value.
        let bare = raw.trim_matches(|c: char| {
            c == '\'' || c == '"' || c == ',' || c == ';' || c == ':' || c == '(' || c == ')'
        });
        if bare.eq_ignore_ascii_case("bearer")
            || bare.eq_ignore_ascii_case("authorization")
            || bare.eq_ignore_ascii_case("authorization:")
        {
            parts.push(raw.to_owned());
            if !bare.contains(':') {
                skip_next = true;
            }
            continue;
        }
        // `key=value` / `key:value` with a secret-looking key.
        let mut redacted: Option<String> = None;
        for sep in ['=', ':'] {
            if let Some(at) = raw.find(sep) {
                let (k, v) = raw.split_at(at);
                let key = k.trim_matches(|c: char| {
                    c == '\'' || c == '"' || c == '-' || c == ',' || c == ';' || c == '('
                });
                if !key.is_empty() && (key_is_secret(key) || token_is_secret(v)) {
                    redacted = Some(format!("{k}{sep}[REDACTED]"));
                    break;
                }
            }
        }
        match redacted {
            Some(r) => parts.push(r),
            None if token_is_secret(raw) => parts.push("[REDACTED]".to_owned()),
            None => parts.push(raw.to_owned()),
        }
    }
    parts.join(" ")
}

/// File-write intent helper for tool arguments that touch the filesystem.
#[must_use]
pub fn file_write_intent(path: &PathBuf) -> OperationIntent {
    OperationIntent::File {
        action: FileAction::Write,
        path: path.clone(),
    }
}

/// Expected digest binding for one intent (grant freshness/version checked by
/// [`Grant::covers`] via [`ExpectedScope`]).
#[must_use]
pub fn digest_of(intent: &OperationIntent) -> OperationDigest {
    OperationDigest::of(intent)
}

#[cfg(test)]
mod tests {
    use super::super::{PermissionSet, SecurityPolicy};
    use super::*;
    use opencode_rk_contracts::SessionId;
    use std::time::{Duration, SystemTime};

    const NOW: u64 = 1_750_000_000;

    fn now() -> SystemTime {
        SystemTime::UNIX_EPOCH + Duration::from_secs(NOW)
    }

    fn broker() -> PermissionBroker {
        PermissionBroker::new(SecurityPolicy::lean_default("/work/project"))
    }

    fn star() -> PermissionBroker {
        broker().with_permissions(PermissionSet::star())
    }

    fn scope_for(b: &PermissionBroker) -> (super::super::app_policy::Scope, ExpectedScope) {
        let session = SessionId::new();
        let scope = super::super::app_policy::Scope::new(
            "/work/project",
            session,
            "local-user",
            now() + Duration::from_secs(3600),
            b.generation(),
        )
        .unwrap();
        let expected = ExpectedScope {
            workspace: PathBuf::from("/work/project"),
            session,
            requester: "local-user".to_owned(),
            now: now(),
            policy_version: b.generation(),
        };
        (scope, expected)
    }

    #[test]
    fn shell_string_requires_human_and_denied_runs_nothing() {
        let b = broker();
        let mut auth = ToolAuthorizer::new(&b);
        let cwd = PathBuf::from("/work/project");
        // T01: executor shell path must broker-authorize; opaque `bash -c`
        // string requires a human gate, never direct execution.
        let gate = auth.authorize(&shell_intent("rm -rf /", &cwd));
        assert!(
            matches!(gate, ToolGate::HumanGate { .. }),
            "opaque shell string must gate, got {gate:?}"
        );
        // Denial-equivalent: a destructive direct-argv intent is Deny, and a
        // denied gate runs zero effects.
        let denied = auth.authorize(&argv_intent(
            "rm",
            &["-rf".to_owned(), "*".to_owned()],
            &cwd,
        ));
        assert!(matches!(denied, ToolGate::Deny { .. }));
        let mut side_effects = 0u32;
        let out = run_if_allowed(&denied, || {
            side_effects += 1;
            42
        });
        assert_eq!(out, None);
        assert_eq!(side_effects, 0, "denied gate executed a side effect");
    }

    #[test]
    fn server_tool_path_denial_has_no_side_effects() {
        let b = broker();
        let mut auth = ToolAuthorizer::new(&b);
        // T02: server tool path authorizes each call; a secret-file write
        // intent denies and produces no file side effects.
        let gate = auth.authorize(&file_write_intent(&PathBuf::from("/etc/hosts")));
        assert!(matches!(gate, ToolGate::Deny { .. }));
        let dir = std::env::temp_dir().join(format!("rk-tool-auth-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let target = dir.join("must-not-exist.txt");
        let outcome = run_if_allowed(&gate, || std::fs::write(&target, b"nope").unwrap());
        assert_eq!(outcome, None);
        assert!(!target.exists(), "denied server tool call left a file");
        let _ = std::fs::remove_dir_all(&dir);
        // Ordinary tool call still authorizes Allow and runs.
        let ok = auth.authorize(&server_tool_intent("read", "read project file"));
        assert_eq!(ok, ToolGate::Allow);
        assert_eq!(run_if_allowed(&ok, || 7), Some(7));
    }

    #[test]
    fn star_cannot_lift_mandatory_across_versions() {
        // T03: `*` cannot lift a mandatory deny under any policy version.
        let base = star();
        let env_intent = OperationIntent::File {
            action: FileAction::Read,
            path: PathBuf::from("/work/project/.env"),
        };
        let mut auth = ToolAuthorizer::new(&base);
        assert!(matches!(auth.authorize(&env_intent), ToolGate::Deny { .. }));
        let repoliced = base.with_policy(SecurityPolicy::lean_default("/work/project"));
        let mut auth2 = ToolAuthorizer::new(&repoliced);
        assert!(
            matches!(auth2.authorize(&env_intent), ToolGate::Deny { .. }),
            "* lifted mandatory deny after version bump"
        );
        // Mandatory human gate also survives `*` without a grant.
        let del = OperationIntent::File {
            action: FileAction::Delete,
            path: PathBuf::from("/work/project/generated.tmp"),
        };
        assert!(matches!(auth2.authorize(&del), ToolGate::HumanGate { .. }));
    }

    #[test]
    fn stale_approval_rejected_without_side_effects() {
        // T04: policy version bump invalidates cached approvals; stale grant
        // rejected with no side effects and no ledger burn observable as Allow.
        let base = broker();
        let intent = OperationIntent::File {
            action: FileAction::Delete,
            path: PathBuf::from("/work/project/generated.tmp"),
        };
        let (scope, _) = scope_for(&base);
        let stale = Grant::new(
            opencode_rk_contracts::ApprovalId::new(),
            digest_of(&intent),
            scope,
            false,
        );
        let repoliced = base.with_policy(SecurityPolicy::lean_default("/work/project"));
        let expected = ExpectedScope {
            workspace: PathBuf::from("/work/project"),
            session: stale.scope.session,
            requester: "local-user".to_owned(),
            now: now(),
            policy_version: repoliced.generation(),
        };
        let mut auth = ToolAuthorizer::new(&repoliced);
        let gate = auth.authorize_with_grant(&intent, &stale, &expected);
        assert!(
            matches!(gate, ToolGate::Deny { .. }),
            "stale approval must deny, got {gate:?}"
        );
        let mut ran = false;
        assert_eq!(
            run_if_allowed(&gate, || {
                ran = true;
            }),
            None
        );
        assert!(!ran, "stale-grant denial ran an effect");
        // Fresh grant for the new version still resumes exactly once.
        let (fresh_scope, fresh_expected) = scope_for(&repoliced);
        let fresh = Grant::new(
            opencode_rk_contracts::ApprovalId::new(),
            digest_of(&intent),
            fresh_scope,
            false,
        );
        assert_eq!(
            auth.authorize_with_grant(&intent, &fresh, &fresh_expected),
            ToolGate::Allow
        );
        assert!(
            matches!(
                auth.authorize_with_grant(&intent, &fresh, &fresh_expected),
                ToolGate::Deny { .. }
            ),
            "grant replay must deny"
        );
    }

    #[test]
    fn denied_calls_audited_with_secrets_redacted() {
        // T05: every wired decision is audited within bound; secrets redacted
        // from the retained log text.
        let b = broker();
        let mut auth = ToolAuthorizer::new(&b);
        let cwd = PathBuf::from("/work/project");
        let secret_cmd =
            "curl -H 'Authorization: Bearer sk-live-1234567890abcdef' https://x.example";
        let _ = auth.authorize(&shell_intent(secret_cmd, &cwd));
        let _ = auth.authorize(&server_tool_intent("read", "ordinary tool call"));
        assert_eq!(auth.audit_len(), 2);
        let trail = auth.audit();
        assert_eq!(trail.iter().map(|e| e.seq).collect::<Vec<_>>(), vec![0, 1]);
        assert_eq!(trail[0].gate, ToolGateSummary::HumanGate);
        assert_eq!(trail[1].gate, ToolGateSummary::Allow);
        for entry in trail {
            assert!(
                !entry.intent_text.contains("sk-live-1234567890abcdef"),
                "secret leaked into audit: {}",
                entry.intent_text
            );
        }
        assert!(
            !redact_secrets("OPENAI_API_KEY=sk-secret-value password=hunter2")
                .contains("sk-secret-value"),
            "redactor leaked secret value"
        );
        // Flood stays bounded.
        for i in 0..(MAX_TOOL_AUDIT + 10) {
            let _ = auth.authorize(&server_tool_intent("t", &format!("call {i}")));
        }
        assert_eq!(auth.audit_len(), MAX_TOOL_AUDIT);
        let seqs: Vec<u64> = auth.audit().iter().map(|e| e.seq).collect();
        assert!(seqs.windows(2).all(|w| w[0] + 1 == w[1]));
    }

    #[test]
    fn redactor_unit() {
        assert_eq!(redact_secrets("plain text").contains("plain"), true);
        assert!(!redact_secrets("token=abc123XYZ").contains("abc123XYZ"));
        assert!(!redact_secrets("sk-live-abc").contains("sk-live-abc"));
    }
}
