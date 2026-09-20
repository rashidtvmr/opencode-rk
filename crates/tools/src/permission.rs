//! Tool permission gate: allowlist + ext grant + secure gate + broker.
//!
//! Deny-unless-authorized composition over the real slices:
//! [`tool_allow`](crate::tool_allow) (exact-match allowlist),
//! [`ext_perms`](crate::ext_perms) (ext, perm) grants,
//! [`ext_secure`](crate::ext_secure) (secure.<perm> grants), and the
//! security [`PermissionBroker`](opencode_rk_security::PermissionBroker).
//!
//! Check order: shape, reserved/human namespace, allowlist (exact match,
//! `*` is literal-only and never expands), ext grant, secure gate,
//! broker. Broker `Deny` is terminal; broker `RequireHuman` is terminal too:
//! no allowlist entry, ext grant, secure grant, or `*` lifts the human gate.
//! Denial runs zero caller effects by construction:
//! [`run_if_authorized`] executes the effect closure only on `Ok(())`.
//!
//! Pure: no spawn, no FS, no clock. `forbid(unsafe_code)`.
#![forbid(unsafe_code)]

use opencode_rk_security::{Decision, OperationIntent, PermissionBroker};
use thiserror::Error;

use crate::ext_perms::ExtPerm;
use crate::ext_secure::SecurePerm;
use crate::tool_allow::is_allowed;

/// Tool permission failures. Every variant is fail-closed.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum PermissionError {
    #[error("tool name is empty")]
    EmptyTool,
    #[error("extension id is empty")]
    EmptyExt,
    #[error("wildcards never grant: {0:?}")]
    WildcardRejected(String),
    #[error("reserved namespace is never grantable: {0:?}")]
    ReservedNamespace(String),
    #[error("tool not allowlisted: {0:?}")]
    NotAllowed(String),
    #[error("extension grant missing: {ext:?} may not exercise {tool:?}")]
    ExtDenied { ext: String, tool: String },
    #[error("secure grant missing: {0:?}")]
    SecureDenied(String),
    #[error("broker denied")]
    BrokerDenied,
    #[error("broker requires human approval")]
    BrokerHumanGate,
}

/// Reserved namespaces never grantable through this gate (mirrors
/// [`crate::app_extensions::grant_input_ok`]).
fn reserved_tool(tool: &str) -> bool {
    ["human.", "system.", "authority.", "grant."]
        .iter()
        .any(|p| tool == p.trim_end_matches('.') || tool.starts_with(*p))
}

/// Authorize one tool call for `ext` through allowlist, ext grants, the
/// secure gate, and the broker, in that order. `intent` is the broker-level
/// intent for the same call; the broker verdict is terminal and `*` never
/// satisfies any layer.
pub fn check(
    allowed_tools: &[String],
    ext_grants: &[ExtPerm],
    secure_grants: &[SecurePerm],
    broker: &PermissionBroker,
    tool: &str,
    ext: &str,
    intent: &OperationIntent,
) -> Result<(), PermissionError> {
    if tool.is_empty() {
        return Err(PermissionError::EmptyTool);
    }
    if ext.is_empty() {
        return Err(PermissionError::EmptyExt);
    }
    if tool.contains('*') || ext.contains('*') {
        return Err(PermissionError::WildcardRejected(tool.to_string()));
    }
    if reserved_tool(tool) {
        return Err(PermissionError::ReservedNamespace(tool.to_string()));
    }
    if !is_allowed(allowed_tools, tool) {
        return Err(PermissionError::NotAllowed(tool.to_string()));
    }
    if !ext_grants.iter().any(|g| g.ext == ext && g.perm == tool) {
        return Err(PermissionError::ExtDenied {
            ext: ext.to_string(),
            tool: tool.to_string(),
        });
    }
    if let Some(perm) = tool.strip_prefix("secure.") {
        let ok = secure_grants
            .iter()
            .any(|s| s.granted && s.name == perm && !s.name.contains('*'));
        if !ok {
            return Err(PermissionError::SecureDenied(tool.to_string()));
        }
    }
    match broker.authorize(intent) {
        Decision::Allow => Ok(()),
        Decision::Deny { .. } => Err(PermissionError::BrokerDenied),
        Decision::RequireHuman { .. } => Err(PermissionError::BrokerHumanGate),
    }
}

/// Run `effect` only when `verdict` is `Ok(())`. Denial and human-gate
/// outcomes never invoke the closure: no process, no file side effects.
pub fn run_if_authorized<T>(
    verdict: &Result<(), PermissionError>,
    effect: impl FnOnce() -> T,
) -> Option<T> {
    match verdict {
        Ok(()) => Some(effect()),
        Err(_) => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use opencode_rk_security::SecurityPolicy;
    use std::path::PathBuf;

    fn broker() -> PermissionBroker {
        PermissionBroker::new(SecurityPolicy::lean_default("/work/project"))
    }

    fn allowed() -> Vec<String> {
        vec!["read".to_string(), "secure.deploy".to_string()]
    }

    fn grants() -> Vec<ExtPerm> {
        vec![ExtPerm {
            ext: "ext-a".to_string(),
            perm: "read".to_string(),
        }]
    }

    fn secure_grants() -> Vec<SecurePerm> {
        vec![SecurePerm {
            name: "deploy".to_string(),
            granted: true,
        }]
    }

    fn read_intent() -> OperationIntent {
        OperationIntent::Tool {
            name: "read".to_string(),
            description: "read project file".to_string(),
        }
    }

    #[test]
    fn t01_allowlisted_granted_broker_allow_ok() {
        let b = broker();
        let mut ext = grants();
        ext.push(ExtPerm {
            ext: "ext-a".to_string(),
            perm: "secure.deploy".to_string(),
        });
        let out = check(
            &allowed(),
            &ext,
            &secure_grants(),
            &b,
            "secure.deploy",
            "ext-a",
            &read_intent(),
        );
        assert_eq!(out, Ok(()));
        assert_eq!(run_if_authorized(&out, || 7), Some(7));
    }

    #[test]
    fn t02_unknown_tool_denied_zero_side_effect() {
        let b = broker();
        let verdict = check(&allowed(), &grants(), &[], &b, "nope-missing", "ext-a", &read_intent());
        assert_eq!(
            verdict,
            Err(PermissionError::NotAllowed("nope-missing".to_string()))
        );
        let mut ran = 0u32;
        assert_eq!(
            run_if_authorized(&verdict, || {
                ran += 1;
                ran
            }),
            None
        );
        assert_eq!(ran, 0);
    }

    #[test]
    fn t03_ungranted_ext_denied_zero_side_effect() {
        let b = broker();
        let verdict = check(&allowed(), &grants(), &[], &b, "read", "ext-b", &read_intent());
        assert_eq!(
            verdict,
            Err(PermissionError::ExtDenied {
                ext: "ext-b".to_string(),
                tool: "read".to_string()
            })
        );
        let mut ran = false;
        assert_eq!(
            run_if_authorized(&verdict, || {
                ran = true;
            }),
            None
        );
        assert!(!ran);
    }

    #[test]
    fn t04_star_never_bypasses_human_gate() {
        let b = broker();
        // Wildcard request rejected even with `*` allowlisted + granted.
        let star_allowed = vec!["*".to_string()];
        let star_grants = vec![ExtPerm {
            ext: "*".to_string(),
            perm: "*".to_string(),
        }];
        let verdict = check(&star_allowed, &star_grants, &[], &b, "*", "*", &read_intent());
        assert_eq!(verdict, Err(PermissionError::WildcardRejected("*".to_string())));
        // Broker mandatory controls survive allowlist + ext grant: secret read
        // denies even when both layers cover the tool.
        let allowed2 = vec!["read".to_string()];
        let grants2 = vec![ExtPerm {
            ext: "ext-a".to_string(),
            perm: "read".to_string(),
        }];
        let secret = OperationIntent::File {
            action: opencode_rk_security::FileAction::Read,
            path: PathBuf::from("/work/project/.env"),
        };
        assert_eq!(
            check(&allowed2, &grants2, &[], &b, "read", "ext-a", &secret),
            Err(PermissionError::BrokerDenied)
        );
        // Mandatory human gate (delete) also holds: no bypass to Allow.
        let del = OperationIntent::File {
            action: opencode_rk_security::FileAction::Delete,
            path: PathBuf::from("/work/project/generated.tmp"),
        };
        assert_eq!(
            check(&allowed2, &grants2, &[], &b, "read", "ext-a", &del),
            Err(PermissionError::BrokerHumanGate)
        );
    }

    #[test]
    fn t05_secure_gate_needs_grant_human_gate_still_holds() {
        let b = broker();
        let mut ext = grants();
        ext.push(ExtPerm {
            ext: "ext-a".to_string(),
            perm: "secure.deploy".to_string(),
        });
        // Ungranted secure perm denies with zero side effects.
        let denied = check(&allowed(), &ext, &[], &b, "secure.deploy", "ext-a", &read_intent());
        assert_eq!(
            denied,
            Err(PermissionError::SecureDenied("secure.deploy".to_string()))
        );
        let mut ran = false;
        assert_eq!(
            run_if_authorized(&denied, || {
                ran = true;
            }),
            None
        );
        assert!(!ran);
        // Granted secure perm still cannot lift the broker human gate.
        let del = OperationIntent::File {
            action: opencode_rk_security::FileAction::Delete,
            path: PathBuf::from("/work/project/generated.tmp"),
        };
        assert_eq!(
            check(&allowed(), &ext, &secure_grants(), &b, "secure.deploy", "ext-a", &del),
            Err(PermissionError::BrokerHumanGate)
        );
        // Reserved human namespace never grantable.
        let human = check(
            &["human.approve".to_string()],
            &[ExtPerm {
                ext: "ext-a".to_string(),
                perm: "human.approve".to_string(),
            }],
            &[],
            &b,
            "human.approve",
            "ext-a",
            &read_intent(),
        );
        assert_eq!(
            human,
            Err(PermissionError::ReservedNamespace(
                "human.approve".to_string()
            ))
        );
    }
}
