//! TDD tests for the typed PROMPT-ASSEMBLY contract (LANE-RULES-INJECT).
//!
//! Pure function: given rules_loader snapshot + rules_globs LoadDecision +
//! bounded base system prompt -> final system prompt string + InjectRecord.
//!
//! Frozen RED: all tests must compile and FAIL before implementation.

#[path = "../src/rules_inject.rs"]
mod rules_inject;

use rules_inject::{assemble_prompt, InjectRecord, LoadedRule, SkippedRule};
use opencode_rk_server::rules_globs::LoadDecision;
use opencode_rk_server::rules_loader::{RuleEntry, RulesSnapshot};
use std::path::PathBuf;

/// T01: Empty rules + empty decision -> base prompt returned with empty record.
#[test]
fn t01_empty_rules_passthrough() {
    let snap = RulesSnapshot { entries: vec![] };
    let decision = LoadDecision {
        load: vec![],
        unload: vec![],
    };
    let (prompt, record) = assemble_prompt(&snap, &decision, "You are a helpful assistant.");
    assert_eq!(prompt, "You are a helpful assistant.");
    assert!(record.loaded.is_empty());
    assert!(record.skipped.is_empty());
}

/// T02: Single always-rule injected with markers in deterministic position.
#[test]
fn t02_single_always_rule_with_markers() {
    let snap = RulesSnapshot {
        entries: vec![RuleEntry {
            path: PathBuf::from("/ws/AGENTS.md"),
            glob: None,
            bytes: b"# Rules\nBe concise.".to_vec(),
        }],
    };
    let decision = LoadDecision {
        load: vec![],
        unload: vec![],
    };
    let (prompt, record) = assemble_prompt(&snap, &decision, "Base prompt.");
    assert!(
        prompt.contains("# Rules\nBe concise."),
        "rule body must appear in prompt"
    );
    assert!(
        prompt.contains("--- rules:AGENTS.md ---"),
        "start marker must be present"
    );
    assert!(
        prompt.contains("--- end rules:AGENTS.md ---"),
        "end marker must be present"
    );
    assert_eq!(record.loaded.len(), 1);
    assert_eq!(record.loaded[0].name, "AGENTS.md");
    assert_eq!(record.loaded[0].bytes, 19);
}

/// T03: Multiple rules injected in deterministic sorted order.
#[test]
fn t03_deterministic_sorted_order() {
    let snap = RulesSnapshot {
        entries: vec![
            RuleEntry {
                path: PathBuf::from("/ws/CLAUDE.md"),
                glob: None,
                bytes: b"claude rules".to_vec(),
            },
            RuleEntry {
                path: PathBuf::from("/ws/AGENTS.md"),
                glob: None,
                bytes: b"agent rules".to_vec(),
            },
            RuleEntry {
                path: PathBuf::from("/ws/rules/zzz.md"),
                glob: None,
                bytes: b"zzz rules".to_vec(),
            },
        ],
    };
    let decision = LoadDecision {
        load: vec![],
        unload: vec![],
    };
    let (prompt, record) = assemble_prompt(&snap, &decision, "Base.");
    assert!(
        prompt.find("agent rules").unwrap() < prompt.find("claude rules").unwrap(),
        "rules must appear in sorted path order"
    );
    assert!(
        prompt.find("claude rules").unwrap() < prompt.find("zzz rules").unwrap(),
        "rules must appear in sorted path order"
    );
    assert_eq!(record.loaded.len(), 3);
}

/// T04: Conditional rule injected only when in LoadDecision::load.
#[test]
fn t04_conditional_rule_injected_when_loaded() {
    let snap = RulesSnapshot {
        entries: vec![
            RuleEntry {
                path: PathBuf::from("/ws/AGENTS.md"),
                glob: None,
                bytes: b"always".to_vec(),
            },
            RuleEntry {
                path: PathBuf::from("/ws/rules/rust.md"),
                glob: Some("**/*.rs".to_string()),
                bytes: b"rust rules".to_vec(),
            },
        ],
    };
    let decision = LoadDecision {
        load: vec!["rust.md".to_string()],
        unload: vec![],
    };
    let (prompt, record) = assemble_prompt(&snap, &decision, "Base.");
    assert!(
        prompt.contains("rust rules"),
        "conditional rule in load set must be injected"
    );
    assert_eq!(record.loaded.len(), 2);
    assert!(record.loaded.iter().any(|r| r.name == "rust.md"));
}

/// T05: Conditional rule skipped when NOT in LoadDecision::load.
#[test]
fn t05_conditional_rule_skipped_when_not_loaded() {
    let snap = RulesSnapshot {
        entries: vec![
            RuleEntry {
                path: PathBuf::from("/ws/AGENTS.md"),
                glob: None,
                bytes: b"always".to_vec(),
            },
            RuleEntry {
                path: PathBuf::from("/ws/rules/rust.md"),
                glob: Some("**/*.rs".to_string()),
                bytes: b"rust rules".to_vec(),
            },
        ],
    };
    let decision = LoadDecision {
        load: vec![],
        unload: vec![],
    };
    let (prompt, record) = assemble_prompt(&snap, &decision, "Base.");
    assert!(
        !prompt.contains("rust rules"),
        "conditional rule NOT in load set must NOT appear in prompt"
    );
    assert_eq!(record.loaded.len(), 1);
    assert_eq!(record.skipped.len(), 1);
    assert_eq!(record.skipped[0].name, "rust.md");
    assert!(record.skipped[0].reason.contains("not in load set"));
}

/// T06: Byte cap enforced -- entry exceeding INJECT_BYTE_CAP is skipped.
#[test]
fn t06_byte_cap_enforced() {
    let big_body = "x".repeat(rules_inject::INJECT_BYTE_CAP + 100);
    let snap = RulesSnapshot {
        entries: vec![RuleEntry {
            path: PathBuf::from("/ws/AGENTS.md"),
            glob: None,
            bytes: big_body.as_bytes().to_vec(),
        }],
    };
    let decision = LoadDecision {
        load: vec![],
        unload: vec![],
    };
    let (prompt, record) = assemble_prompt(&snap, &decision, "Base.");
    assert_eq!(record.loaded.len(), 0, "over-budget entry must not load");
    assert_eq!(record.skipped.len(), 1, "over-budget entry must be skipped");
    assert!(record.skipped[0].reason.contains("budget"));
    assert_eq!(prompt, "Base.");
}

/// T07: Determinism -- same inputs always produce same prompt and record.
#[test]
fn t07_determinism_across_runs() {
    let snap = RulesSnapshot {
        entries: vec![
            RuleEntry {
                path: PathBuf::from("/ws/B.md"),
                glob: None,
                bytes: b"B content".to_vec(),
            },
            RuleEntry {
                path: PathBuf::from("/ws/A.md"),
                glob: None,
                bytes: b"A content".to_vec(),
            },
        ],
    };
    let decision = LoadDecision {
        load: vec![],
        unload: vec![],
    };
    let (p1, r1) = assemble_prompt(&snap, &decision, "Base.");
    let (p2, r2) = assemble_prompt(&snap, &decision, "Base.");
    assert_eq!(p1, p2, "prompt must be deterministic");
    assert_eq!(r1, r2, "record must be deterministic");
}

/// T08: Glob field preserved in loaded record.
#[test]
fn t08_glob_preserved_in_loaded_record() {
    let snap = RulesSnapshot {
        entries: vec![RuleEntry {
            path: PathBuf::from("/ws/rules/python.md"),
            glob: Some("**/*.py".to_string()),
            bytes: b"python rules".to_vec(),
        }],
    };
    let decision = LoadDecision {
        load: vec!["python.md".to_string()],
        unload: vec![],
    };
    let (_, record) = assemble_prompt(&snap, &decision, "Base.");
    assert_eq!(record.loaded.len(), 1);
    assert_eq!(record.loaded[0].glob.as_deref(), Some("**/*.py"));
}

/// T09: Base prompt appears AFTER all injected rules.
#[test]
fn t09_base_prompt_after_rules() {
    let snap = RulesSnapshot {
        entries: vec![RuleEntry {
            path: PathBuf::from("/ws/AGENTS.md"),
            glob: None,
            bytes: b"rule content".to_vec(),
        }],
    };
    let decision = LoadDecision {
        load: vec![],
        unload: vec![],
    };
    let (prompt, _) = assemble_prompt(&snap, &decision, "BASE PROMPT");
    let rules_pos = prompt.find("rule content").unwrap();
    let base_pos = prompt.find("BASE PROMPT").unwrap();
    assert!(
        rules_pos < base_pos,
        "rules must appear before base prompt"
    );
}

/// T10: Skipped reason includes "budget exceeded" when cap hit.
#[test]
fn t10_budget_exceeded_skip_reason() {
    let entry1_bytes = vec![b'a'; rules_inject::INJECT_BYTE_CAP - 10];
    let entry2_bytes = vec![b'b'; 100];
    let snap = RulesSnapshot {
        entries: vec![
            RuleEntry {
                path: PathBuf::from("/ws/first.md"),
                glob: None,
                bytes: entry1_bytes,
            },
            RuleEntry {
                path: PathBuf::from("/ws/second.md"),
                glob: None,
                bytes: entry2_bytes,
            },
        ],
    };
    let decision = LoadDecision {
        load: vec![],
        unload: vec![],
    };
    let (prompt, record) = assemble_prompt(&snap, &decision, "Base.");
    assert_eq!(record.loaded.len(), 1);
    assert_eq!(record.loaded[0].name, "first.md");
    assert_eq!(record.skipped.len(), 1);
    assert_eq!(record.skipped[0].name, "second.md");
    assert!(record.skipped[0].reason.contains("budget"));
    assert!(
        !prompt.contains("bbb"),
        "budget-excluded rule must not appear"
    );
}

/// T11: Record size bounded -- MAX_RECORD_ENTRIES enforced.
#[test]
fn t11_record_size_bounded() {
    let entries: Vec<RuleEntry> = (0..rules_inject::MAX_RECORD_ENTRIES + 10)
        .map(|i| RuleEntry {
            path: PathBuf::from(format!("/ws/rules/r{:04}.md", i)),
            glob: None,
            bytes: b"content".to_vec(),
        })
        .collect();
    let snap = RulesSnapshot { entries };
    let decision = LoadDecision {
        load: vec![],
        unload: vec![],
    };
    let (_, record) = assemble_prompt(&snap, &decision, "Base.");
    let total = record.loaded.len() + record.skipped.len();
    assert!(
        total <= rules_inject::MAX_RECORD_ENTRIES,
        "record total {} exceeds cap {}",
        total,
        rules_inject::MAX_RECORD_ENTRIES
    );
}
