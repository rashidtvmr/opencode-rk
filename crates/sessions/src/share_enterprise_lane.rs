//! Enterprise-remote boundary, fallback lane (SHARE-003 T01..T05).
//!
//! Explicit declaration: the native binary performs no hosted share/sync,
//! support/GitHub/deployment side effects. Every enterprise-remote operation
//! resolves to a typed refusal naming the missing in-surface specification
//! partition (see `sources/enterprise-remote-spec-gap.json`, status
//! `searched-no-qualifying-in-surface-spec`, missing kind `spec`).
//!
//! Lane note: this file (`share_enterprise_lane.rs`) is the WRITE-lane owned
//! fallback copy. It is intentionally NOT wired into `lib.rs` (integrator
//! assembles shared files) and is exercised via `#[path]` include from
//! `crates/sessions/tests/share_enterprise_lane.rs`. It mirrors the
//! `share_enterprise.rs` boundary contract without sharing its module so the
//! two files never collide.
//!
//! Local secret-free lifecycle only: pure synchronous predicate over one
//! caller-owned [`EnterpriseOp`] value. No I/O, no network, no clock, no
//! tasks, no retained state, no secret/credential input, no DB writes.
#![forbid(unsafe_code)]

use std::fmt;

/// Enterprise-remote operation attempted against the native binary.
///
/// Each variant maps to exactly one required gap partition
/// (see [`partition`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EnterpriseOp {
    /// Hosted enterprise share HTTP (`enterprise-share-http`).
    /// Legacy-vs-org endpoint selection is refused here, never defaulted.
    ShareHttp,
    /// Function SyncServer WebSocket/R2 relay
    /// (`function-syncserver-websocket-r2`).
    SyncServerRelay,
    /// Function support relay, including support-admin removal authority
    /// (`function-support-relay`). No bearer check is performed locally.
    SupportRelay,
    /// Function GitHub token exchange / installation lookup
    /// (`function-github-token-exchange-installation`).
    GithubTokenExchange,
    /// Hosted deployment resource lifecycle
    /// (`deployment-resource-lifecycle`).
    DeploymentLifecycle,
}

/// Typed refusal boundary error.
///
/// Carries only op + partition + reason names. No credentials, secrets,
/// URLs, tokens, or endpoints.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BoundaryError {
    /// The attempted op is refused: the owning spec partition is missing.
    Refused {
        /// The attempted op, echoed back (caller-owned copy).
        op: EnterpriseOp,
        /// Missing in-surface specification partition name.
        partition: &'static str,
        /// Why: cites the spec-gap file and names the missing `spec` kind.
        reason: &'static str,
    },
}

impl fmt::Display for BoundaryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Refused {
                op,
                partition,
                reason,
            } => write!(
                f,
                "enterprise op {op:?} refused under partition {partition}: {reason}"
            ),
        }
    }
}

impl std::error::Error for BoundaryError {}

/// Map an op to its exact required gap partition.
///
/// Order matches
/// `sources/enterprise-remote-spec-gap.json::requiredDecompositionPartitions`.
#[must_use]
pub const fn partition(op: EnterpriseOp) -> &'static str {
    match op {
        EnterpriseOp::ShareHttp => "enterprise-share-http",
        EnterpriseOp::SyncServerRelay => "function-syncserver-websocket-r2",
        EnterpriseOp::SupportRelay => "function-support-relay",
        EnterpriseOp::GithubTokenExchange => "function-github-token-exchange-installation",
        EnterpriseOp::DeploymentLifecycle => "deployment-resource-lifecycle",
    }
}

/// Refusal reason for an op: cites the spec-gap file and names the missing
/// `spec` kind. Carries no endpoint, URL, or credential.
#[must_use]
pub const fn refusal_reason(op: EnterpriseOp) -> &'static str {
    match op {
        EnterpriseOp::ShareHttp => "missing in-surface `spec` for partition `enterprise-share-http` (see `sources/enterprise-remote-spec-gap.json`); legacy-vs-org endpoint selection refused, no endpoint chosen",
        EnterpriseOp::SyncServerRelay => "missing in-surface `spec` for partition `function-syncserver-websocket-r2` (see `sources/enterprise-remote-spec-gap.json`)",
        EnterpriseOp::SupportRelay => "missing in-surface `spec` for partition `function-support-relay` (see `sources/enterprise-remote-spec-gap.json`); support-admin removal authority refused; no credential check performed locally",
        EnterpriseOp::GithubTokenExchange => "missing in-surface `spec` for partition `function-github-token-exchange-installation` (see `sources/enterprise-remote-spec-gap.json`)",
        EnterpriseOp::DeploymentLifecycle => "missing in-surface `spec` for partition `deployment-resource-lifecycle` (see `sources/enterprise-remote-spec-gap.json`)",
    }
}

/// Explicit native-side enterprise-remote boundary.
///
/// Refusal precedes any side effect: this function performs no I/O before
/// returning `Err`, so there is nothing to roll back, retry, or replay.
pub struct EnterpriseBoundary;

impl EnterpriseBoundary {
    /// Refuse `op`: always `Err(BoundaryError::Refused)` naming the op,
    /// its partition, and the reason. Terminal for the op; the caller
    /// decides the next safe local step.
    pub fn authorize(op: EnterpriseOp) -> Result<(), BoundaryError> {
        Err(BoundaryError::Refused {
            op,
            partition: partition(op),
            reason: refusal_reason(op),
        })
    }
}
