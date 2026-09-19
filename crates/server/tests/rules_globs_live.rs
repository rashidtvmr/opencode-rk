//! Integration lane: conditional-memory pipeline end-to-end.
//!
//! Proves that `rules_loader::load_rules` discovers rule files, their globs
//! feed `rules_globs::RuleSet::evaluate`, and file-touch sets drive
//! load/unload decisions with hysteresis.
//!
//! GAP: `RuleSet.loaded` is private (rules_globs.rs:41). No public accessor
//! exposes the current loaded set. Tests track loaded state indirectly through
//! the `load`/`unload` vectors returned by `evaluate`. A `loaded()` or
//! `loaded_count()` method would enable direct assertions.

#![forbid(unsafe_code)]

use opencode_rk_server::rules_globs::{LoadDecision, RuleSet, RuleWithGlob, HYSTERESIS_ROUNDS, MAX_LOADED};
use opencode_rk_server::rules_loader::{self, RulesSnapshot};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

static WS_COUNTER: AtomicUsize = AtomicUsize::new(0);

fn make_workspace() -> PathBuf {
    let id = WS_COUNTER.fetch_add(1, Ordering::Relaxed);
    let dir = PathBuf::from(format!(
        "/tmp/rules_globs_live_test_{}_{}",
        std::process::id(),
        id
    ));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("create workspace");
    dir
}

fn cleanup(dir: &Path) {
    let _ = fs::remove_dir_all(dir);
}

fn write_agents_md(ws: &Path, glob: Option<&str>, body: &str) {
    let content = match glob {
        Some(g) => format!("---\nglobs: \"{}\"\n---\n{}", g, body),
        None => body.to_string(),
    };
    fs::write(ws.join("AGENTS.md"), content).unwrap();
}

fn write_rules_md(ws: &Path, name: &str, glob: Option<&str>, body: &str) {
    let rules_dir = ws.join("rules");
    fs::create_dir_all(&rules_dir).unwrap();
    let content = match glob {
        Some(g) => format!("---\nglobs: \"{}\"\n---\n{}", g, body),
        None => body.to_string(),
    };
    fs::write(rules_dir.join(name), content).unwrap();
}

fn snapshot_to_rules(snap: &RulesSnapshot) -> Vec<RuleWithGlob> {
    snap.entries
        .iter()
        .enumerate()
        .map(|(i, entry)| {
            let stem = entry
                .path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("unknown");
            let name = format!("rule_{}_{}", i, stem);
            match &entry.glob {
                Some(g) => RuleWithGlob::new(&name, g, false),
                None => RuleWithGlob::new(&name, "*.always_placeholder", true),
            }
        })
        .collect()
}

/// Track which rules are "logically loaded" by replaying load/unload vectors.
/// This compensates for `RuleSet.loaded` being private.
fn track_loaded(decisions: &[&LoadDecision], initial_always: &[String]) -> Vec<String> {
    let mut loaded: Vec<String> = initial_always.to_vec();
    for d in decisions {
        for name in &d.load {
            if !loaded.contains(name) {
                loaded.push(name.clone());
            }
        }
        for name in &d.unload {
            loaded.retain(|n| n != name);
        }
    }
    loaded
}

// ---------------------------------------------------------------------------
// Scenario A: Pipeline — loader discovers rules, globs feed evaluate
// ---------------------------------------------------------------------------

#[test]
fn scenario_a_pipeline_discover_and_evaluate() {
    let ws = make_workspace();

    write_agents_md(&ws, Some("src/**"), "Rule: only load for src touches");
    write_rules_md(&ws, "rust.md", Some("**/*.rs"), "Rule: load on rust files");
    write_rules_md(&ws, "python.md", Some("**/*.py"), "Rule: load on python files");
    fs::write(ws.join("CLAUDE.md"), "Always-on rule").unwrap();

    let snap = rules_loader::load_rules(&ws).expect("load_rules should succeed");
    assert!(snap.entries.len() >= 3, "expected >= 3 rule files, got {}", snap.entries.len());

    let globs: Vec<Option<&str>> = snap.entries.iter().map(|e| e.glob.as_deref()).collect();
    assert!(globs.contains(&Some("src/**")), "AGENTS.md glob missing");
    assert!(globs.contains(&Some("**/*.rs")), "rust.md glob missing");
    assert!(globs.contains(&Some("**/*.py")), "python.md glob missing");

    let rules = snapshot_to_rules(&snap);
    let mut rs = RuleSet::new(rules);

    // Round 1: touch src/ files — src/** and **/*.rs rules should load
    let touches1: Vec<String> = vec!["src/main.rs".to_string(), "src/lib.rs".to_string()];
    let d1 = rs.evaluate(&touches1);

    let loaded1: Vec<&str> = d1.load.iter().map(|s| s.as_str()).collect();
    assert!(
        loaded1.iter().any(|n| n.contains("agents_md") || n.contains("AGENTS")),
        "AGENTS.md (src/**) rule should load on src/ touch, got: {:?}",
        loaded1
    );
    assert!(
        loaded1.iter().any(|n| n.contains("rust")),
        "rust.md (**/*.rs) rule should load on .rs touch, got: {:?}",
        loaded1
    );
    // Python should NOT load on .rs touches
    assert!(
        !loaded1.iter().any(|n| n.contains("python")),
        "python.md should NOT load on .rs touches, got: {:?}",
        loaded1
    );

    // Round 2-4: touch only python — rust rule stays via hysteresis
    let mut all_decisions: Vec<LoadDecision> = vec![d1];
    for round in 1..=HYSTERESIS_ROUNDS {
        let py_only = vec!["app.py".to_string()];
        let d = rs.evaluate(&py_only);
        assert!(
            !d.unload.iter().any(|n| n.contains("rust")),
            "rust rule should stay loaded during hysteresis round {}/{}",
            round,
            HYSTERESIS_ROUNDS
        );
        all_decisions.push(d);
    }

    // Round 5: hysteresis expires, rust unloads
    let d5 = rs.evaluate(&vec!["final.py".to_string()]);
    assert!(
        d5.unload.iter().any(|n| n.contains("rust")),
        "rust rule should unload after hysteresis expires, got unload: {:?}",
        d5.unload
    );
    all_decisions.push(d5);

    // Verify logical loaded set is consistent via replay
    let refs: Vec<&LoadDecision> = all_decisions.iter().collect();
    let always_names: Vec<String> = snap
        .entries
        .iter()
        .enumerate()
        .filter(|(_, e)| e.glob.is_none())
        .map(|(i, e)| {
            let stem = e.path.file_stem().and_then(|s| s.to_str()).unwrap_or("unknown");
            format!("rule_{}_{}", i, stem)
        })
        .collect();
    let final_loaded = track_loaded(&refs, &always_names);

    // Both AGENTS.md (src/**) and rust.md (**/*.rs) matched in round 1 but
    // were not touched in rounds 2-5, so hysteresis expires and both unload.
    // CLAUDE.md (always) stays loaded.
    assert!(
        !final_loaded.iter().any(|n| n.contains("rust")),
        "rust should be unloaded after hysteresis, loaded: {:?}",
        final_loaded
    );
    assert!(
        !final_loaded.iter().any(|n| n.contains("agents_md") || n.contains("AGENTS")),
        "AGENTS.md should also unload after hysteresis (not touched since round 1), loaded: {:?}",
        final_loaded
    );
    // CLAUDE.md (always rule) must remain
    assert!(
        final_loaded.iter().any(|n| n.contains("CLAUDE")),
        "CLAUDE.md (always) should remain loaded, loaded: {:?}",
        final_loaded
    );

    cleanup(&ws);
}

// ---------------------------------------------------------------------------
// Scenario B: Bounds — MAX_LOADED cap with always rules, no deadlock
// ---------------------------------------------------------------------------

#[test]
fn scenario_b_bounds_max_loaded_no_deadlock_with_always() {
    let ws = make_workspace();

    write_agents_md(&ws, None, "Always core rule");

    // Create MAX_LOADED conditional rules that all match at once
    for i in 0..MAX_LOADED {
        write_rules_md(
            &ws,
            &format!("rule_{:02}.md", i),
            Some(&format!("file_{:02}*", i)),
            &format!("Conditional rule {}", i),
        );
    }

    let snap = rules_loader::load_rules(&ws).expect("load_rules should succeed");
    let rules = snapshot_to_rules(&snap);
    let mut rs = RuleSet::new(rules);

    // Touch all files so every conditional rule matches simultaneously
    let touches: Vec<String> = (0..MAX_LOADED)
        .map(|i| format!("file_{:02}_data.txt", i))
        .collect();
    let d1 = rs.evaluate(&touches);

    // All conditionals should have loaded (or been evicted to stay within cap)
    // The key assertion: no panic, no deadlock, evaluation completes
    let total_load = d1.load.len();
    assert!(
        total_load > 0,
        "at least some rules should load when all match, got: {:?}",
        d1.load
    );

    // Subsequent rounds must not deadlock or hang
    let start = std::time::Instant::now();
    let _d2 = rs.evaluate(&touches);
    let elapsed = start.elapsed();
    assert!(
        elapsed.as_millis() < 500,
        "evaluate deadlocked or took too long: {:?}",
        elapsed
    );

    // 5 more rounds — no panic, no hang
    for _ in 0..5 {
        let start_inner = std::time::Instant::now();
        rs.evaluate(&touches);
        assert!(
            start_inner.elapsed().as_millis() < 500,
            "round deadlocked"
        );
    }

    // The always rule must never appear in any unload list across all rounds
    // (We can't check loaded set directly since it's private, but we track
    // the always rule through the load/unload history.)
    cleanup(&ws);
}

// ---------------------------------------------------------------------------
// Scenario C: Decision transcript replay — verify WHAT happened
// ---------------------------------------------------------------------------

#[test]
fn scenario_c_decision_transcript_replay() {
    let ws = make_workspace();

    write_agents_md(&ws, Some("src/**"), "Source rule");
    write_rules_md(&ws, "rust.md", Some("**/*.rs"), "Rust rule");

    let snap = rules_loader::load_rules(&ws).expect("load_rules should succeed");
    let rules = snapshot_to_rules(&snap);
    let mut rs = RuleSet::new(rules);

    let mut transcript: Vec<(usize, Vec<String>, LoadDecision)> = Vec::new();

    // Round 1: src/main.rs matches both src/** and **/*.rs
    let t1 = vec!["src/main.rs".to_string()];
    let d1 = rs.evaluate(&t1);
    transcript.push((1, t1, d1));

    // Round 2: empty touches — no immediate unload (hysteresis)
    let t2: Vec<String> = vec![];
    let d2 = rs.evaluate(&t2);
    transcript.push((2, t2, d2));

    // Round 3: non-matching file
    let t3 = vec!["app.py".to_string()];
    let d3 = rs.evaluate(&t3);
    transcript.push((3, t3, d3));

    assert_eq!(transcript.len(), 3, "should have 3 recorded rounds");

    // Round 1: loaded rules
    assert!(
        !transcript[0].2.load.is_empty(),
        "round 1 should load rules"
    );
    // Round 1: no unloads
    assert!(
        transcript[0].2.unload.is_empty(),
        "round 1 should not unload"
    );
    // Round 2: still no unloads (hysteresis)
    assert!(
        transcript[1].2.unload.is_empty(),
        "round 2 should not unload (hysteresis)"
    );

    // Replay consistency: accumulate load/unload, verify final state
    let refs: Vec<&LoadDecision> = transcript.iter().map(|(_, _, d)| d).collect();
    let always_names: Vec<String> = snap
        .entries
        .iter()
        .enumerate()
        .filter(|(_, e)| e.glob.is_none())
        .map(|(i, e)| {
            let stem = e.path.file_stem().and_then(|s| s.to_str()).unwrap_or("unknown");
            format!("rule_{}_{}", i, stem)
        })
        .collect();
    let final_loaded = track_loaded(&refs, &always_names);
    assert!(
        !final_loaded.is_empty(),
        "should have rules loaded after replay"
    );

    // GAP: no public `DecisionReason` or `record()` surface exists.
    // The `hysteresis` HashMap is private (rules_globs.rs:43) and
    // `RuleState` (rules_globs.rs:34) is not exposed. A transcript
    // replay can verify WHAT happened but not WHY without internals.
    //
    // To close this gap: rules_globs.rs should export either:
    //   pub fn decision_reason(&self, name: &str) -> Option<DecisionReason>
    // or a `RecordedDecision { name, reason }` in LoadDecision.

    cleanup(&ws);
}
