//! App-level policy decision envelope (PAR-006).
//!
//! Binds an explicit human grant to exactly one [`OperationDigest`] inside one
//! [`Scope`] (workspace, session, requester, expiry, policy version) and maps a
//! [`PermissionBroker`] verdict to an app-level [`AppDecision`].
//!
//! Baseline-first semantics, consistent with
//! `crates/security/src/lib.rs:202-223 authorize` and `star_proof.rs`: the
//! broker verdict is authoritative. A mandatory broker `Deny` is never lifted
//! by a grant, and `*`/wildcard permission scope cannot bypass it. A grant only
//! satisfies `RequireHuman`, and only when digest, scope, freshness, version
//! and single-use replay checks all hold. Stale versions and replays are
//! rejected fail-closed. No OS sandbox is claimed here: [`require_os_sandbox`]
//! always returns [`PolicyDeny::UnsupportedSandbox`].
#![forbid(unsafe_code)]

use opencode_rk_contracts::{ApprovalId, SessionId};
use super::{Decision, FileAction, OperationIntent, PermissionBroker};
use serde::{Deserialize, Serialize};
use std::{
    collections::{HashSet, VecDeque},
    fmt,
    path::{Component, Path, PathBuf},
    time::SystemTime,
};
use thiserror::Error;

/// Schema marker for the [`Scope`] layout consumed by [`Grant::covers`].
pub const APP_POLICY_SCOPE_VERSION: u32 = 1;
/// Upper bound on `requester` identity bytes (AGENTS.md: byte budgets).
pub const MAX_REQUESTER_BYTES: usize = 256;
/// Upper bound on consumed-grant replay memory (AGENTS.md: no unbounded state).
pub const MAX_USED_GRANTS: usize = 4096;

/// Content binding for one authorized operation: FNV-1a-64 over the canonical
/// encoding of an [`OperationIntent`]. Any changed byte (path, argv, SQL text,
/// tool input) yields a different digest, so a grant cannot be retargeted.
///
// ponytail: FNV-1a is binding-only, not collision-resistant; upgrade to a
// cryptographic digest (e.g. blake3) when a new Cargo dependency is approved.
#[derive(Clone, Debug, Eq, PartialEq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct OperationDigest(String);

impl OperationDigest {
    #[must_use]
    pub fn of(intent: &OperationIntent) -> Self {
        Self(format!("{:016x}", fnv1a64(canonical(intent).as_bytes())))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl fmt::Display for OperationDigest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

/// Authority scope of one explicit human grant. Work through [`Scope::new`]:
/// empty or wildcard (`*`) workspace/requester values are rejected fail-closed
/// so a broad pattern can never become a bypass.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Scope {
    pub workspace: PathBuf,
    pub session: SessionId,
    pub requester: String,
    pub expires_at: SystemTime,
    pub policy_version: u64,
}

impl Scope {
    pub fn new(
        workspace: impl Into<PathBuf>,
        session: SessionId,
        requester: impl Into<String>,
        expires_at: SystemTime,
        policy_version: u64,
    ) -> Result<Self, PolicyDeny> {
        let workspace = workspace.into();
        if workspace.as_os_str().is_empty() {
            return Err(PolicyDeny::InvalidScope(
                "workspace must not be empty".to_owned(),
            ));
        }
        if workspace.to_string_lossy().contains('*') {
            return Err(PolicyDeny::InvalidScope(
                "wildcard workspace scope rejected".to_owned(),
            ));
        }
        let requester = requester.into();
        if requester.is_empty() {
            return Err(PolicyDeny::InvalidScope(
                "requester must not be empty".to_owned(),
            ));
        }
        if requester.contains('*') {
            return Err(PolicyDeny::InvalidScope(
                "wildcard requester scope rejected".to_owned(),
            ));
        }
        if requester.len() > MAX_REQUESTER_BYTES {
            return Err(PolicyDeny::InvalidScope(
                "requester identity exceeds byte budget".to_owned(),
            ));
        }
        Ok(Self {
            workspace,
            session,
            requester,
            expires_at,
            policy_version,
        })
    }
}

/// Explicit human grant: one approval id bound to one digest inside one scope.
/// `local_only` marks grants that must never be honored from a remote origin;
/// see [`enforce_local_only`].
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Grant {
    pub approval: ApprovalId,
    pub digest: OperationDigest,
    pub scope: Scope,
    pub local_only: bool,
}

impl Grant {
    #[must_use]
    pub fn new(
        approval: ApprovalId,
        digest: OperationDigest,
        scope: Scope,
        local_only: bool,
    ) -> Self {
        Self {
            approval,
            digest,
            scope,
            local_only,
        }
    }

    /// Check digest, freshness, version and scope binding. Replay is checked by
    /// [`GrantLedger`] in [`decide`], not here, so a failed check never burns a
    /// single-use grant.
    pub fn covers(
        &self,
        intent: &OperationIntent,
        expected: &ExpectedScope,
    ) -> Result<(), PolicyDeny> {
        if expected.now > self.scope.expires_at {
            return Err(PolicyDeny::Expired);
        }
        if self.scope.policy_version != expected.policy_version {
            return Err(PolicyDeny::StalePolicyVersion {
                expected: expected.policy_version,
                got: self.scope.policy_version,
            });
        }
        if self.digest.is_empty() || OperationDigest::of(intent) != self.digest {
            return Err(PolicyDeny::DigestMismatch);
        }
        if lexical_normalize(&expected.workspace) != lexical_normalize(&self.scope.workspace) {
            return Err(PolicyDeny::ScopeMismatch("workspace mismatch".to_owned()));
        }
        if expected.session != self.scope.session {
            return Err(PolicyDeny::ScopeMismatch("session mismatch".to_owned()));
        }
        if expected.requester != self.scope.requester {
            return Err(PolicyDeny::ScopeMismatch("requester mismatch".to_owned()));
        }
        Ok(())
    }
}

/// Present-tense execution context a grant is checked against. `now` is passed
/// explicitly so tests never depend on the wall clock.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExpectedScope {
    pub workspace: PathBuf,
    pub session: SessionId,
    pub requester: String,
    pub now: SystemTime,
    pub policy_version: u64,
}

/// App-level decision. `HumanOnly` means "no covering grant; a human must still
/// approve". `Deny` is terminal for this attempt: mandatory broker denials,
/// expired/stale/replayed grants and remote use of local-only grants.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "decision", rename_all = "snake_case")]
pub enum AppDecision {
    Allow,
    Deny { reason: String },
    HumanOnly { reason: String },
}

impl AppDecision {
    #[must_use]
    pub fn is_allow(&self) -> bool {
        matches!(self, Self::Allow)
    }

    #[must_use]
    pub fn is_mandatory(&self) -> bool {
        matches!(self, Self::Deny { .. } | Self::HumanOnly { .. })
    }
}

/// Fail-closed rejection reasons for grant validation.
#[derive(Clone, Debug, Eq, PartialEq, Error, Serialize, Deserialize)]
#[serde(tag = "reason", rename_all = "snake_case")]
pub enum PolicyDeny {
    #[error("grant scope invalid: {0}")]
    InvalidScope(String),
    #[error("grant expired")]
    Expired,
    #[error("stale grant: issued for policy {got}, current is {expected}")]
    StalePolicyVersion { expected: u64, got: u64 },
    #[error("operation digest mismatch")]
    DigestMismatch,
    #[error("grant scope mismatch: {0}")]
    ScopeMismatch(String),
    #[error("grant replay rejected")]
    Replay,
    #[error("grant ledger exhausted")]
    LedgerFull,
    #[error("mandatory denial holds; grant cannot lift: {0}")]
    Mandatory(String),
    #[error("local-only grant presented from non-local origin")]
    RemoteOrigin,
    #[error("unsupported OS sandbox backend: {0}")]
    UnsupportedSandbox(String),
}

/// Single-use replay ledger: consumed approval ids. Bounded at
/// [`MAX_USED_GRANTS`]; overflow fails closed with [`PolicyDeny::LedgerFull`]
/// rather than evicting (which would reopen replay).
#[derive(Clone, Debug, Default)]
pub struct GrantLedger {
    used: HashSet<ApprovalId>,
    order: VecDeque<ApprovalId>,
}

impl GrantLedger {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.used.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.used.is_empty()
    }

    #[must_use]
    pub fn contains(&self, id: &ApprovalId) -> bool {
        self.used.contains(id)
    }

    pub fn consume(&mut self, id: ApprovalId) -> Result<(), PolicyDeny> {
        if self.used.contains(&id) {
            return Err(PolicyDeny::Replay);
        }
        if self.used.len() >= MAX_USED_GRANTS {
            return Err(PolicyDeny::LedgerFull);
        }
        self.used.insert(id);
        self.order.push_back(id);
        Ok(())
    }
}

/// Baseline-first app decision. The broker runs first; only `RequireHuman` may
/// be satisfied by a covering, fresh, version-current, single-use grant.
/// Mandatory `Deny` is never lifted, with or without a grant, under `*` or not.
pub fn decide(
    broker: &PermissionBroker,
    intent: &OperationIntent,
    grant: Option<&Grant>,
    expected: &ExpectedScope,
    ledger: &mut GrantLedger,
) -> AppDecision {
    match broker.authorize(intent) {
        Decision::Allow => AppDecision::Allow,
        Decision::Deny { reason } => AppDecision::Deny {
            reason: PolicyDeny::Mandatory(reason).to_string(),
        },
        Decision::RequireHuman { reason, .. } => match grant {
            None => AppDecision::HumanOnly { reason },
            Some(grant) => match grant.covers(intent, expected) {
                Ok(()) => {
                    if ledger.contains(&grant.approval) {
                        return AppDecision::Deny {
                            reason: PolicyDeny::Replay.to_string(),
                        };
                    }
                    match ledger.consume(grant.approval) {
                        Ok(()) => AppDecision::Allow,
                        Err(deny @ (PolicyDeny::Replay | PolicyDeny::LedgerFull)) => {
                            AppDecision::Deny {
                                reason: deny.to_string(),
                            }
                        }
                        Err(deny) => AppDecision::Deny {
                            reason: deny.to_string(),
                        },
                    }
                }
                // Fail-closed grant defects: expired, stale version. A bad grant
                // never degrades into silent escalation, and never burns replay.
                Err(
                    deny @ (PolicyDeny::Expired
                    | PolicyDeny::StalePolicyVersion { .. }
                    | PolicyDeny::InvalidScope(_)),
                ) => AppDecision::Deny {
                    reason: deny.to_string(),
                },
                // Grant simply does not cover this operation (wrong digest or
                // scope): stay at human approval, do not deny the operation.
                Err(_) => AppDecision::HumanOnly { reason },
            },
        },
    }
}

/// Local-only enforcement: a `local_only` grant presented from a non-local
/// (e.g. remote) origin is denied, never escalated.
#[must_use]
pub fn enforce_local_only(
    is_local_origin: bool,
    grant: &Grant,
    decision: AppDecision,
) -> AppDecision {
    if grant.local_only && !is_local_origin {
        AppDecision::Deny {
            reason: PolicyDeny::RemoteOrigin.to_string(),
        }
    } else {
        decision
    }
}

/// OS isolation gate. Never claims a sandbox: without a tested, linked OS
/// backend this crate cannot isolate, so it fails closed and says so honestly
/// (PAR-006: an unsupported sandbox capability must fail closed, not hide
/// behind a regex or prompt).
pub fn require_os_sandbox(backend: Option<&str>) -> Result<(), PolicyDeny> {
    Err(PolicyDeny::UnsupportedSandbox(match backend {
        None => "no OS isolation backend configured for this process".to_owned(),
        Some(name) => format!(
            "backend `{name}` is not a tested in-process OS sandbox; refusing to claim isolation"
        ),
    }))
}

fn fnv1a64(bytes: &[u8]) -> u64 {
    const OFFSET: u64 = 0xcbf29ce484222325;
    const PRIME: u64 = 0x100000001b3;
    let mut hash = OFFSET;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(PRIME);
    }
    hash
}

fn canonical(intent: &OperationIntent) -> String {
    const SEP: char = '\0';
    match intent {
        OperationIntent::File { action, path } => {
            let act = match action {
                FileAction::Read => "read",
                FileAction::Write => "write",
                FileAction::Delete => "delete",
                FileAction::CreateDirectory => "mkdir",
            };
            format!("file{SEP}{act}{SEP}{}", flat_path(path))
        }
        OperationIntent::Process { program, args, cwd } => {
            format!(
                "process{SEP}{program}{SEP}{}{SEP}{}",
                args.join(&SEP.to_string()),
                flat_path(cwd)
            )
        }
        OperationIntent::Sql {
            statement,
            database,
        } => format!("sql{SEP}{statement}{SEP}{database}"),
        OperationIntent::Tool { name, description } => {
            format!("tool{SEP}{name}{SEP}{description}")
        }
    }
}

fn flat_path(path: &Path) -> String {
    lexical_normalize(path).to_string_lossy().replace('\\', "/")
}

fn lexical_normalize(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                out.pop();
            }
            other => out.push(other.as_os_str()),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::{PermissionSet, SecurityPolicy};
    use std::time::Duration;

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

    fn delete_tmp() -> OperationIntent {
        OperationIntent::File {
            action: FileAction::Delete,
            path: PathBuf::from("/work/project/generated.tmp"),
        }
    }

    fn read_env() -> OperationIntent {
        OperationIntent::File {
            action: FileAction::Read,
            path: PathBuf::from("/work/project/.env"),
        }
    }

    fn write_src() -> OperationIntent {
        OperationIntent::File {
            action: FileAction::Write,
            path: PathBuf::from("/work/project/src/lib.rs"),
        }
    }

    fn ctx(policy_version: u64) -> (Scope, ExpectedScope) {
        let session = SessionId::new();
        let scope = Scope::new(
            "/work/project",
            session,
            "local-user",
            now() + Duration::from_secs(3600),
            policy_version,
        )
        .unwrap();
        let expected = ExpectedScope {
            workspace: PathBuf::from("/work/project"),
            session,
            requester: "local-user".to_owned(),
            now: now(),
            policy_version,
        };
        (scope, expected)
    }

    fn grant_for(intent: &OperationIntent, scope: &Scope, local_only: bool) -> Grant {
        Grant::new(
            ApprovalId::new(),
            OperationDigest::of(intent),
            scope.clone(),
            local_only,
        )
    }

    #[test]
    fn baseline_allow_needs_no_grant() {
        let b = star();
        let (_, expected) = ctx(b.generation());
        let mut ledger = GrantLedger::new();
        assert_eq!(decide(&b, &write_src(), None, &expected, &mut ledger), AppDecision::Allow);
        assert!(ledger.is_empty());
    }

    #[test]
    fn mandatory_deny_not_lifted_by_matching_grant() {
        // star + digest-matching grant must not bypass secret-file protection.
        let b = star();
        let (scope, expected) = ctx(b.generation());
        let intent = read_env();
        let grant = grant_for(&intent, &scope, false);
        let mut ledger = GrantLedger::new();
        assert!(matches!(
            decide(&b, &intent, Some(&grant), &expected, &mut ledger),
            AppDecision::Deny { .. }
        ));
        assert!(ledger.is_empty());
    }

    #[test]
    fn star_cannot_bypass_human_gate_without_grant() {
        let b = star();
        let (_, expected) = ctx(b.generation());
        let mut ledger = GrantLedger::new();
        assert!(matches!(
            decide(&b, &delete_tmp(), None, &expected, &mut ledger),
            AppDecision::HumanOnly { .. }
        ));
    }

    #[test]
    fn valid_grant_satisfies_human_gate_once() {
        let b = broker();
        let (scope, expected) = ctx(b.generation());
        let intent = delete_tmp();
        let grant = grant_for(&intent, &scope, false);
        let mut ledger = GrantLedger::new();
        assert_eq!(
            decide(&b, &intent, Some(&grant), &expected, &mut ledger),
            AppDecision::Allow
        );
        assert!(ledger.contains(&grant.approval));
    }

    #[test]
    fn replay_invalid() {
        let b = broker();
        let (scope, expected) = ctx(b.generation());
        let intent = delete_tmp();
        let grant = grant_for(&intent, &scope, false);
        let mut ledger = GrantLedger::new();
        assert_eq!(
            decide(&b, &intent, Some(&grant), &expected, &mut ledger),
            AppDecision::Allow
        );
        let again = decide(&b, &intent, Some(&grant), &expected, &mut ledger);
        assert!(
            matches!(&again, AppDecision::Deny { reason } if reason.contains("replay")),
            "replay must deny, got {again:?}"
        );
    }

    #[test]
    fn stale_policy_version_rejected() {
        let base = broker();
        let (scope, _) = ctx(base.generation());
        let intent = delete_tmp();
        let grant = grant_for(&intent, &scope, false);
        let repoliced = base.with_policy(SecurityPolicy::lean_default("/work/project"));
        let expected = ExpectedScope {
            workspace: PathBuf::from("/work/project"),
            session: scope.session,
            requester: "local-user".to_owned(),
            now: now(),
            policy_version: repoliced.generation(),
        };
        let mut ledger = GrantLedger::new();
        let decision = decide(&repoliced, &intent, Some(&grant), &expected, &mut ledger);
        assert!(
            matches!(&decision, AppDecision::Deny { reason } if reason.contains("stale")),
            "stale grant must deny, got {decision:?}"
        );
        assert!(ledger.is_empty());
    }

    #[test]
    fn expired_grant_rejected() {
        let b = broker();
        let session = SessionId::new();
        let scope = Scope::new(
            "/work/project",
            session,
            "local-user",
            now() - Duration::from_secs(1),
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
        let intent = delete_tmp();
        let grant = grant_for(&intent, &scope, false);
        let mut ledger = GrantLedger::new();
        assert!(
            matches!(
                decide(&b, &intent, Some(&grant), &expected, &mut ledger),
                AppDecision::Deny { .. }
            )
        );
        assert!(ledger.is_empty());
    }

    #[test]
    fn digest_mismatch_stays_human() {
        let b = broker();
        let (scope, expected) = ctx(b.generation());
        let other = OperationIntent::File {
            action: FileAction::Delete,
            path: PathBuf::from("/work/project/other.tmp"),
        };
        let grant = grant_for(&other, &scope, false);
        let mut ledger = GrantLedger::new();
        assert!(matches!(
            decide(&b, &delete_tmp(), Some(&grant), &expected, &mut ledger),
            AppDecision::HumanOnly { .. }
        ));
        assert!(ledger.is_empty());
    }

    #[test]
    fn scope_mismatch_stays_human() {
        let b = broker();
        let (scope, _) = ctx(b.generation());
        let intent = delete_tmp();
        let grant = grant_for(&intent, &scope, false);
        let expected = ExpectedScope {
            workspace: PathBuf::from("/work/project"),
            session: SessionId::new(),
            requester: "local-user".to_owned(),
            now: now(),
            policy_version: b.generation(),
        };
        let mut ledger = GrantLedger::new();
        assert!(matches!(
            decide(&b, &intent, Some(&grant), &expected, &mut ledger),
            AppDecision::HumanOnly { .. }
        ));
        assert!(ledger.is_empty());
    }

    #[test]
    fn wildcard_scope_rejected() {
        let session = SessionId::new();
        let far = now() + Duration::from_secs(60);
        assert!(matches!(
            Scope::new("*", session, "local-user", far, 1),
            Err(PolicyDeny::InvalidScope(_))
        ));
        assert!(matches!(
            Scope::new("/work/**", session, "*", far, 1),
            Err(PolicyDeny::InvalidScope(_))
        ));
        assert!(matches!(
            Scope::new("", session, "local-user", far, 1),
            Err(PolicyDeny::InvalidScope(_))
        ));
        assert!(matches!(
            Scope::new("/work/project", session, "", far, 1),
            Err(PolicyDeny::InvalidScope(_))
        ));
    }

    #[test]
    fn local_only_remote_denied() {
        let b = broker();
        let (scope, expected) = ctx(b.generation());
        let intent = delete_tmp();
        let grant = grant_for(&intent, &scope, true);
        let mut ledger = GrantLedger::new();
        let decision = decide(&b, &intent, Some(&grant), &expected, &mut ledger);
        assert_eq!(decision, AppDecision::Allow);
        assert!(
            matches!(
                enforce_local_only(false, &grant, decision),
                AppDecision::Deny { .. }
            )
        );
        let decision = decide(&b, &intent, None, &expected, &mut GrantLedger::new());
        let _ = enforce_local_only(true, &grant, decision);
        // Fresh grant honored from local origin.
        let grant2 = grant_for(&intent, &scope, true);
        let mut ledger2 = GrantLedger::new();
        let decision2 = decide(&b, &intent, Some(&grant2), &expected, &mut ledger2);
        assert_eq!(
            enforce_local_only(true, &grant2, decision2),
            AppDecision::Allow
        );
    }

    #[test]
    fn unsupported_sandbox_fail_closed() {
        assert!(matches!(
            require_os_sandbox(None),
            Err(PolicyDeny::UnsupportedSandbox(_))
        ));
        assert!(matches!(
            require_os_sandbox(Some("landlock")),
            Err(PolicyDeny::UnsupportedSandbox(_))
        ));
    }

    #[test]
    fn digest_binding_stable_and_sensitive() {
        let a = OperationDigest::of(&delete_tmp());
        let b = OperationDigest::of(&delete_tmp());
        assert_eq!(a, b);
        let changed = OperationDigest::of(&OperationIntent::File {
            action: FileAction::Delete,
            path: PathBuf::from("/work/project/generated.tmp "),
        });
        assert_ne!(a, changed);
        let sql_a = OperationDigest::of(&OperationIntent::Sql {
            statement: "DELETE FROM users WHERE id = 7".to_owned(),
            database: "workspace".to_owned(),
        });
        let sql_b = OperationDigest::of(&OperationIntent::Sql {
            statement: "DELETE FROM users".to_owned(),
            database: "workspace".to_owned(),
        });
        assert_ne!(sql_a, sql_b);
    }

    #[test]
    fn grant_serde_round_trip() {
        let (_, _) = ctx(1);
        let b = broker();
        let (scope, _) = ctx(b.generation());
        let grant = grant_for(&delete_tmp(), &scope, true);
        let json = serde_json::to_string(&grant).unwrap();
        assert_eq!(serde_json::from_str::<Grant>(&json).unwrap(), grant);
        let deny = AppDecision::Deny {
            reason: "x".to_owned(),
        };
        let json = serde_json::to_string(&deny).unwrap();
        assert_eq!(serde_json::from_str::<AppDecision>(&json).unwrap(), deny);
    }

    #[test]
    fn ledger_full_fail_closed() {
        let mut ledger = GrantLedger::new();
        for _ in 0..MAX_USED_GRANTS {
            ledger.consume(ApprovalId::new()).unwrap();
        }
        assert_eq!(
            ledger.consume(ApprovalId::new()),
            Err(PolicyDeny::LedgerFull)
        );
    }
}
