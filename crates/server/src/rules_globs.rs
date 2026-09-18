#![forbid(unsafe_code)]

use std::collections::{HashMap, VecDeque};

#[derive(Debug, Clone)]
pub struct RuleWithGlob {
    pub name: String,
    pub glob: String,
    pub always: bool,
}

impl RuleWithGlob {
    pub fn new(name: &str, glob: &str, always: bool) -> Self {
        assert!(name.len() <= 64, "name exceeds 64 bytes");
        assert!(glob.len() <= 128, "glob exceeds 128 bytes");
        Self {
            name: name.to_string(),
            glob: glob.to_string(),
            always,
        }
    }
}

#[derive(Debug, Clone)]
pub struct LoadDecision {
    pub load: Vec<String>,
    pub unload: Vec<String>,
}

pub const HYSTERESIS_ROUNDS: usize = 3;
pub const MAX_LOADED: usize = 32;

#[derive(Debug, Clone)]
struct RuleState {
    last_match_round: i64,
    round_started_loaded: bool,
}

#[derive(Debug)]
pub struct RuleSet {
    rules: Vec<RuleWithGlob>,
    loaded: Vec<String>,
    hysteresis: HashMap<String, RuleState>,
    load_order: VecDeque<String>,
    round: i64,
}

impl RuleSet {
    pub fn new(rules: Vec<RuleWithGlob>) -> Self {
        let mut rs = Self {
            rules,
            loaded: Vec::new(),
            hysteresis: HashMap::new(),
            load_order: VecDeque::new(),
            round: 0,
        };
        rs.apply_always_loaded();
        rs
    }

    fn apply_always_loaded(&mut self) {
        for rule in &self.rules {
            if rule.always && !self.loaded.contains(&rule.name) {
                self.loaded.push(rule.name.clone());
                self.load_order.push_back(rule.name.clone());
            }
        }
    }

    pub fn evaluate(&mut self, touches: &[String]) -> LoadDecision {
        self.round += 1;

        // Phase 1: compute matches without mutating self
        struct MatchInfo {
            name: String,
            matched: bool,
            is_loaded: bool,
        }
        let mut matches: Vec<MatchInfo> = Vec::new();
        {
            for rule in &self.rules {
                if rule.always {
                    continue;
                }
                let matched = touches.iter().any(|t| GlobMatcher::matches(&rule.glob, t));
                let is_loaded = self.loaded.contains(&rule.name);
                matches.push(MatchInfo {
                    name: rule.name.clone(),
                    matched,
                    is_loaded,
                });
            }
        }

        // Phase 2: mutate based on matches
        let mut load = Vec::new();
        let mut unload = Vec::new();
        let mut matched_names: Vec<String> = Vec::new();

        for m in &matches {
            if m.matched {
                matched_names.push(m.name.clone());
                self.hysteresis.insert(
                    m.name.clone(),
                    RuleState {
                        last_match_round: self.round,
                        round_started_loaded: m.is_loaded,
                    },
                );
            }
            if m.matched && !m.is_loaded {
                self.evict_if_full();
                self.loaded.push(m.name.clone());
                self.load_order.push_back(m.name.clone());
                load.push(m.name.clone());
            }
        }

        for name in &self.loaded {
            if self.is_always(name) {
                continue;
            }
            if !matched_names.contains(name) {
                if let Some(state) = self.hysteresis.get(name) {
                    let rounds_since = self.round - state.last_match_round;
                    if rounds_since > HYSTERESIS_ROUNDS as i64 {
                        unload.push(name.clone());
                    }
                } else {
                    unload.push(name.clone());
                }
            }
        }

        self.loaded.retain(|n| !unload.contains(n));
        for evicted in &unload {
            self.load_order.retain(|n| n != evicted);
            self.hysteresis.remove(evicted);
        }

        LoadDecision { load, unload }
    }

    fn is_always(&self, name: &str) -> bool {
        self.rules.iter().any(|r| r.name == name && r.always)
    }

    fn evict_if_full(&mut self) {
        // Evict the oldest NON-always rule. Always rules are rotated to the
        // back of load_order (they can never be evicted) so they cannot
        // permanently block the queue front; if every loaded rule is
        // 'always' the cap is unreachable and we stop (bounded).
        let mut rotated = 0usize;
        while self.loaded.len() >= MAX_LOADED && rotated <= self.load_order.len() {
            let Some(oldest) = self.load_order.pop_front() else {
                break;
            };
            if self.is_always(&oldest) {
                self.load_order.push_back(oldest);
                rotated += 1;
                continue;
            }
            self.loaded.retain(|n| n != &oldest);
            self.hysteresis.remove(&oldest);
        }
    }
}

pub struct GlobMatcher;

impl GlobMatcher {
    pub fn matches(glob: &str, path: &str) -> bool {
        Self::match_inner(glob.as_bytes(), path.as_bytes(), 0, 0, 0)
    }

    fn match_inner(g: &[u8], p: &[u8], gi: usize, pi: usize, star_steps: usize) -> bool {
        const MAX_STAR_STEPS: usize = 64;
        if star_steps > MAX_STAR_STEPS {
            return false;
        }

        if gi == g.len() {
            return pi == p.len();
        }

        if g[gi] == b'*' {
            if gi + 1 < g.len() && g[gi + 1] == b'*' {
                // Double star: matches any chars including /
                if gi + 2 < g.len() && g[gi + 2] == b'/' {
                    // **/ pattern: skip the /**/ in glob, try matching at every position in path
                    let rest_gi = gi + 3;
                    // Try matching rest of glob against every suffix of path
                    for end in pi..=p.len() {
                        if Self::match_inner(g, p, rest_gi, end, star_steps + 1) {
                            return true;
                        }
                    }
                    return false;
                } else {
                    // ** at end or before non-/
                    let rest_gi = gi + 2;
                    for end in pi..=p.len() {
                        if Self::match_inner(g, p, rest_gi, end, star_steps + 1) {
                            return true;
                        }
                    }
                    return false;
                }
            } else {
                // Single star: matches any chars except /
                let rest_gi = gi + 1;
                for end in pi..=p.len() {
                    if end > pi && p[end - 1] == b'/' {
                        break;
                    }
                    if Self::match_inner(g, p, rest_gi, end, star_steps + 1) {
                        return true;
                    }
                }
                return false;
            }
        }

        if pi < p.len() && (g[gi] == b'?' || g[gi] == p[pi]) {
            return Self::match_inner(g, p, gi + 1, pi + 1, star_steps);
        }

        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn t01_single_star_segment_match() {
        assert!(GlobMatcher::matches("*.rs", "foo.rs"));
        assert!(GlobMatcher::matches("src/*.rs", "src/main.rs"));
        assert!(!GlobMatcher::matches("*.rs", "foo.txt"));
        assert!(!GlobMatcher::matches("*.rs", "dir/foo.rs"));
    }

    #[test]
    fn t02_double_star_cross_segment() {
        assert!(GlobMatcher::matches("**/*.rs", "src/main.rs"));
        assert!(GlobMatcher::matches("**/*.rs", "deep/nested/file.rs"));
        assert!(GlobMatcher::matches("**/*.rs", "file.rs"));
        assert!(!GlobMatcher::matches("**/*.rs", "file.txt"));
        assert!(GlobMatcher::matches("src/**", "src/deep/nested/file.rs"));
    }

    #[test]
    fn t03_question_mark() {
        assert!(GlobMatcher::matches("file?.txt", "file1.txt"));
        assert!(GlobMatcher::matches("file?.txt", "fileA.txt"));
        assert!(!GlobMatcher::matches("file?.txt", "file12.txt"));
        assert!(!GlobMatcher::matches("file?.txt", "file.txt"));
    }

    #[test]
    fn t04_hysteresis_holds_3_rounds_then_unloads() {
        let rules = vec![
            RuleWithGlob::new("rust", "*.rs", false),
            RuleWithGlob::new("python", "*.py", false),
        ];
        let mut rs = RuleSet::new(rules);

        // Round 1: touch rust files -> rust loads
        let d1 = rs.evaluate(&["main.rs".to_string()]);
        assert!(d1.load.contains(&"rust".to_string()));
        assert!(d1.unload.is_empty());

        // Round 2: touch python only -> rust stays (hysteresis round 1)
        let d2 = rs.evaluate(&["app.py".to_string()]);
        assert!(!d2.unload.contains(&"rust".to_string()));

        // Round 3: touch python only -> rust stays (hysteresis round 2)
        let d3 = rs.evaluate(&["lib.py".to_string()]);
        assert!(!d3.unload.contains(&"rust".to_string()));

        // Round 4: touch python only -> rust stays (hysteresis round 3)
        let d4 = rs.evaluate(&["run.py".to_string()]);
        assert!(!d4.unload.contains(&"rust".to_string()));

        // Round 5: touch python only -> rust unloads (hysteresis expired)
        let d5 = rs.evaluate(&["util.py".to_string()]);
        assert!(d5.unload.contains(&"rust".to_string()));
    }

    #[test]
    fn t05_always_loaded_never_unloads() {
        let rules = vec![
            RuleWithGlob::new("core", "*.rs", true),
            RuleWithGlob::new("dynamic", "*.py", false),
        ];
        let mut rs = RuleSet::new(rules);

        // Core always loaded from start
        assert!(rs.loaded.contains(&"core".to_string()));

        // Load dynamic
        rs.evaluate(&["app.py".to_string()]);

        // Touch nothing matching dynamic for many rounds
        for _ in 0..10 {
            let d = rs.evaluate(&[]);
            assert!(!d.unload.contains(&"core".to_string()));
            assert!(rs.loaded.contains(&"core".to_string()));
        }
    }

    #[test]
    fn t06_overflow_evicts_oldest_non_always() {
        let mut rules: Vec<RuleWithGlob> = (0..31)
            .map(|i| RuleWithGlob::new(&format!("r{}", i), &format!("f{}.txt", i), false))
            .collect();
        // Add an always rule that should not be evicted
        rules.push(RuleWithGlob::new("always", "always*.txt", true));

        let mut rs = RuleSet::new(rules);

        // Load all 31 non-always rules (1 always + 31 = 32 = MAX_LOADED)
        let touches: Vec<String> = (0..31).map(|i| format!("f{}.txt", i)).collect();
        rs.evaluate(&touches);
        assert_eq!(rs.loaded.len(), MAX_LOADED);

        // Now force overflow by loading one more
        let extra_rules = vec![RuleWithGlob::new("extra", "extra.txt", false)];
        rs.rules.extend(extra_rules);
        rs.evaluate(&["extra.txt".to_string()]);

        // always rule must survive
        assert!(rs.loaded.contains(&"always".to_string()));
        // Total loaded <= MAX_LOADED
        assert!(rs.loaded.len() <= MAX_LOADED);
        // Oldest non-always rule evicted
        assert!(!rs.loaded.contains(&"r0".to_string()));
    }

    #[test]
    fn t07_bounded_backtracking() {
        // Pathological pattern: **a**b - should not hang
        let start = std::time::Instant::now();
        let _ = GlobMatcher::matches("**a**b", &"a".repeat(200));
        let elapsed = start.elapsed();
        assert!(
            elapsed.as_millis() < 100,
            "Backtracking took too long: {:?}",
            elapsed
        );

        let start2 = std::time::Instant::now();
        assert!(!GlobMatcher::matches("**a**b", &"x".repeat(200)));
        let elapsed2 = start2.elapsed();
        assert!(
            elapsed2.as_millis() < 100,
            "Backtracking took too long: {:?}",
            elapsed2
        );
    }

    #[test]
    fn t08_deterministic_across_shuffled_touch_order() {
        let rules = vec![
            RuleWithGlob::new("a", "*.a", false),
            RuleWithGlob::new("b", "*.b", false),
            RuleWithGlob::new("c", "*.c", false),
        ];

        // Run 1: order A, B, C
        let mut rs1 = RuleSet::new(rules.clone());
        let touches1 = vec![
            "x.a".to_string(),
            "y.b".to_string(),
            "z.c".to_string(),
        ];
        let d1 = rs1.evaluate(&touches1);

        // Run 2: order C, B, A
        let mut rs2 = RuleSet::new(rules);
        let touches2 = vec![
            "z.c".to_string(),
            "y.b".to_string(),
            "x.a".to_string(),
        ];
        let d2 = rs2.evaluate(&touches2);

        assert_eq!(d1.load.len(), d2.load.len());
        assert_eq!(d1.unload.len(), d2.unload.len());

        // Both should have loaded all three
        let mut load1 = d1.load.clone();
        let mut load2 = d2.load.clone();
        load1.sort();
        load2.sort();
        assert_eq!(load1, load2);
    }
}
