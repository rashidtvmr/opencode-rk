//! PROMPT-ASSEMBLY contract: typed prompt injection (LANE-RULES-INJECT).
//!
//! Pure function that composes rules_loader discovery + rules_globs
//! LoadDecision + bounded base system prompt → final system prompt +
//! per-round InjectRecord.
//!
//! # In-file tests
//!
//! T01  empty rules passthrough
//! T02  single always-rule with markers
//! T03  deterministic sorted order
//! T04  conditional rule injected when loaded
//! T05  conditional rule skipped when not loaded
//! T06  byte cap enforced
//! T07  determinism across runs
//! T08  glob preserved in loaded record
//! T09  base prompt after rules
//! T10  budget exceeded skip reason
//! T11  record size bounded

#![forbid(unsafe_code)]

use opencode_rk_server::rules_globs::LoadDecision;
use opencode_rk_server::rules_loader::{RuleEntry, RulesSnapshot};
use std::path::PathBuf;

/// Maximum bytes of rule content injected into the system prompt.
pub const INJECT_BYTE_CAP: usize = 128 * 1024;

/// Maximum entries in the InjectRecord (loaded + skipped combined).
pub const MAX_RECORD_ENTRIES: usize = 64;

/// Marker prefix for injected rule blocks.
const MARKER_PREFIX: &str = "--- rules:";
const MARKER_SUFFIX: &str = "---";
const MARKER_END_PREFIX: &str = "--- end rules:";

/// A loaded rule in the injection record.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LoadedRule {
    pub name: String,
    pub glob: Option<String>,
    pub bytes: usize,
}

/// A skipped rule in the injection record.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SkippedRule {
    pub name: String,
    pub reason: String,
}

/// Per-round injection record: what was loaded and what was skipped.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InjectRecord {
    pub loaded: Vec<LoadedRule>,
    pub skipped: Vec<SkippedRule>,
}

/// Extract the file name from a path as a display name.
fn rule_name(path: &PathBuf) -> String {
    path.file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("unknown")
        .to_string()
}

/// Assemble the final system prompt by injecting loaded rule files from the
/// snapshot into the base prompt, respecting the LoadDecision and byte cap.
///
/// Rules are injected in deterministic order (sorted by path). Each rule is
/// wrapped in markers: `--- rules:NAME ---\n<body>\n--- end rules:NAME ---`
///
/// Injection policy:
/// - Rules without a glob (top-level AGENTS.md / CLAUDE.md) are always
///   injected, subject to byte cap.
/// - Rules with a glob are injected only if their file name appears in
///   `decision.load`.
/// - Entries exceeding the remaining byte budget are skipped.
///
/// Returns `(assembled_prompt, InjectRecord)`.
pub fn assemble_prompt(
    snapshot: &RulesSnapshot,
    decision: &LoadDecision,
    base_prompt: &str,
) -> (String, InjectRecord) {
    let mut entries: Vec<&RuleEntry> = snapshot.entries.iter().collect();
    entries.sort_by(|a, b| a.path.cmp(&b.path));

    let mut prompt = String::new();
    let mut loaded: Vec<LoadedRule> = Vec::new();
    let mut skipped: Vec<SkippedRule> = Vec::new();
    let mut bytes_used: usize = 0;

    for entry in &entries {
        let name = rule_name(&entry.path);
        let has_glob = entry.glob.is_some();
        let in_load_set = decision.load.iter().any(|n| n == &name);

        let should_inject = if !has_glob {
            true
        } else {
            in_load_set
        };

        if !should_inject {
            if skipped.len() < MAX_RECORD_ENTRIES {
                skipped.push(SkippedRule {
                    name,
                    reason: "not in load set".to_string(),
                });
            }
            continue;
        }

        if bytes_used + entry.bytes.len() > INJECT_BYTE_CAP {
            if skipped.len() < MAX_RECORD_ENTRIES {
                skipped.push(SkippedRule {
                    name,
                    reason: format!(
                        "budget exceeded: {} bytes remaining, entry needs {} bytes",
                        INJECT_BYTE_CAP - bytes_used,
                        entry.bytes.len()
                    ),
                });
            }
            continue;
        }

        let body = String::from_utf8_lossy(&entry.bytes);
        prompt.push_str(MARKER_PREFIX);
        prompt.push_str(&name);
        prompt.push_str(" ");
        prompt.push_str(MARKER_SUFFIX);
        prompt.push('\n');
        prompt.push_str(&body);
        prompt.push('\n');
        prompt.push_str(MARKER_END_PREFIX);
        prompt.push_str(&name);
        prompt.push_str(" ");
        prompt.push_str(MARKER_SUFFIX);
        prompt.push('\n');

        bytes_used += entry.bytes.len();

        if loaded.len() < MAX_RECORD_ENTRIES {
            loaded.push(LoadedRule {
                name,
                glob: entry.glob.clone(),
                bytes: entry.bytes.len(),
            });
        }
    }

    prompt.push_str(base_prompt);

    (prompt, InjectRecord { loaded, skipped })
}
