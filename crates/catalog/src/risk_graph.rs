//! Change-risk queries over a bounded local symbol graph (TOOL-018).
//!
//! Advisory index: every cited `file:line` must be re-verified against source
//! before acting. No IO, no threads, no clock. Bounded BFS only.
//!
//! Edge convention: `Edge { from, to }` means `to` directly depends on
//! `from`. `blast_radius`/`why` walk forward (`from` -> `to`, toward
//! dependents); `dep_path(a, b)` walks forward from `a` to `b`.
#![forbid(unsafe_code)]

use std::collections::{BTreeMap, VecDeque};
use thiserror::Error;

/// Symbol definition: owner `file:line` + kind.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SymDef {
    pub name: String,
    pub kind: String,
    pub file: String,
    pub line: u32,
}

/// Dependency edge: `to` directly depends on `from`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Edge {
    pub from: String,
    pub to: String,
}

/// Resource bounds for the index and its queries.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RiskCaps {
    pub max_symbols: usize,
    pub max_edges: usize,
    pub max_nodes: usize,
    pub max_depth: usize,
    pub enabled: bool,
}

impl Default for RiskCaps {
    fn default() -> Self {
        Self {
            max_symbols: 20_000,
            max_edges: 100_000,
            max_nodes: 200,
            max_depth: 8,
            enabled: true,
        }
    }
}

/// One dependent with BFS distance from the queried symbol.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BlastMember {
    pub name: String,
    pub depth: usize,
}

/// Bounded change-risk estimate: direct + transitive dependents, capped.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BlastRadius {
    pub members: Vec<BlastMember>,
    pub truncated: bool,
}

/// Owner explanation for one symbol.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Why {
    pub def: SymDef,
    pub dependents: Vec<String>,
    pub dependents_truncated: bool,
}

/// Query failures. BFS limits truncate/flag instead of hard-erroring.
#[derive(Clone, Debug, PartialEq, Eq, Error)]
pub enum RiskError {
    #[error("unknown symbol: {0}")]
    UnknownSymbol(String),
    #[error("self edge")]
    SelfEdge,
    #[error("index disabled")]
    Disabled,
    #[error("capability cap exceeded")]
    CapExceeded,
}

/// Bounded in-memory symbol graph. Two vecs + adjacency map only.
#[derive(Clone, Debug, Default)]
pub struct RiskIndex {
    caps: RiskCaps,
    defs: BTreeMap<String, SymDef>,
    fwd: BTreeMap<String, Vec<String>>,
    edge_count: usize,
}

impl RiskIndex {
    #[must_use]
    pub fn new(caps: RiskCaps) -> Self {
        Self {
            caps,
            defs: BTreeMap::new(),
            fwd: BTreeMap::new(),
            edge_count: 0,
        }
    }

    #[must_use]
    pub fn disabled() -> Self {
        Self {
            caps: RiskCaps {
                enabled: false,
                ..RiskCaps::default()
            },
            defs: BTreeMap::new(),
            fwd: BTreeMap::new(),
            edge_count: 0,
        }
    }

    fn check_on(&self) -> Result<(), RiskError> {
        if self.caps.enabled {
            Ok(())
        } else {
            Err(RiskError::Disabled)
        }
    }

    /// Upserts a symbol; duplicate name overwrites, count unchanged.
    pub fn index_symbol(&mut self, def: SymDef) -> Result<(), RiskError> {
        if !self.caps.enabled {
            return Ok(());
        }
        if self.defs.contains_key(&def.name) {
            self.defs.insert(def.name.clone(), def);
            return Ok(());
        }
        if self.defs.len() >= self.caps.max_symbols {
            return Err(RiskError::CapExceeded);
        }
        self.fwd.entry(def.name.clone()).or_default();
        self.defs.insert(def.name.clone(), def);
        Ok(())
    }

    /// Records `to` depends on `from`. Duplicate edge ignored.
    pub fn index_edge(&mut self, edge: Edge) -> Result<(), RiskError> {
        if !self.caps.enabled {
            return Ok(());
        }
        if edge.from == edge.to {
            return Err(RiskError::SelfEdge);
        }
        if !self.defs.contains_key(&edge.from) {
            return Err(RiskError::UnknownSymbol(edge.from));
        }
        if !self.defs.contains_key(&edge.to) {
            return Err(RiskError::UnknownSymbol(edge.to));
        }
        if self
            .fwd
            .get(&edge.from)
            .is_some_and(|v| v.iter().any(|t| t == &edge.to))
        {
            return Ok(());
        }
        if self.edge_count >= self.caps.max_edges {
            return Err(RiskError::CapExceeded);
        }
        self.fwd.entry(edge.from.clone()).or_default().push(edge.to);
        self.edge_count += 1;
        Ok(())
    }

    /// BFS over forward edges from `name`; capped at `max_nodes`.
    pub fn blast_radius(&self, name: &str) -> Result<BlastRadius, RiskError> {
        self.check_on()?;
        if !self.defs.contains_key(name) {
            return Err(RiskError::UnknownSymbol(name.to_owned()));
        }
        let mut members = Vec::new();
        let mut visited: BTreeMap<&str, usize> = BTreeMap::new();
        visited.insert(name, 0);
        let mut queue: VecDeque<(&str, usize)> = VecDeque::new();
        queue.push_back((name, 0));
        let mut truncated = false;
        while let Some((cur, depth)) = queue.pop_front() {
            if let Some(nexts) = self.fwd.get(cur) {
                for nxt in nexts {
                    if visited.contains_key(nxt.as_str()) {
                        continue;
                    }
                    if members.len() >= self.caps.max_nodes {
                        truncated = true;
                        break;
                    }
                    visited.insert(nxt.as_str(), depth + 1);
                    members.push(BlastMember {
                        name: nxt.clone(),
                        depth: depth + 1,
                    });
                    queue.push_back((nxt.as_str(), depth + 1));
                }
                if truncated {
                    break;
                }
            }
        }
        // One more dependent beyond the cap may hide behind an empty
        // adjacency list; re-scan cheaply: if any unvisited reachable
        // dependent remains, flag truncation.
        if !truncated {
            'outer: for (from, tos) in &self.fwd {
                if !visited.contains_key(from.as_str()) {
                    continue;
                }
                for t in tos {
                    if !visited.contains_key(t.as_str()) {
                        truncated = true;
                        break 'outer;
                    }
                }
            }
        }
        members.sort_by(|a, b| (a.depth, &a.name).cmp(&(b.depth, &b.name)));
        Ok(BlastRadius { members, truncated })
    }

    /// Owner `file:line` plus direct dependents, capped at `max_nodes`.
    pub fn why(&self, name: &str) -> Result<Why, RiskError> {
        self.check_on()?;
        let def = self
            .defs
            .get(name)
            .cloned()
            .ok_or_else(|| RiskError::UnknownSymbol(name.to_owned()))?;
        let mut dependents: Vec<String> = self
            .fwd
            .get(name)
            .cloned()
            .unwrap_or_default();
        dependents.sort();
        let dependents_truncated = dependents.len() > self.caps.max_nodes;
        dependents.truncate(self.caps.max_nodes);
        Ok(Why {
            def,
            dependents,
            dependents_truncated,
        })
    }

    /// One short forward path `[a..b]` within `max_depth` edges.
    pub fn dep_path(&self, a: &str, b: &str) -> Result<Option<Vec<String>>, RiskError> {
        self.check_on()?;
        if !self.defs.contains_key(a) {
            return Err(RiskError::UnknownSymbol(a.to_owned()));
        }
        if !self.defs.contains_key(b) {
            return Err(RiskError::UnknownSymbol(b.to_owned()));
        }
        if a == b {
            return Ok(Some(vec![a.to_owned()]));
        }
        let mut prev: BTreeMap<&str, &str> = BTreeMap::new();
        let mut depth: BTreeMap<&str, usize> = BTreeMap::new();
        depth.insert(a, 0);
        let mut queue: VecDeque<&str> = VecDeque::new();
        queue.push_back(a);
        while let Some(cur) = queue.pop_front() {
            let d = depth[cur];
            if d >= self.caps.max_depth {
                continue;
            }
            if let Some(nexts) = self.fwd.get(cur) {
                for nxt in nexts {
                    if depth.contains_key(nxt.as_str()) {
                        continue;
                    }
                    depth.insert(nxt.as_str(), d + 1);
                    prev.insert(nxt.as_str(), cur);
                    if nxt == b {
                        let mut path = vec![b.to_owned()];
                        let mut at = b;
                        while at != a {
                            at = prev[at];
                            path.push(at.to_owned());
                        }
                        path.reverse();
                        return Ok(Some(path));
                    }
                    queue.push_back(nxt.as_str());
                }
            }
        }
        Ok(None)
    }

    /// Symbols defined in `file`, in line order. Pure local scan, no FS.
    #[must_use]
    pub fn owner_of(&self, file: &str) -> Vec<&SymDef> {
        if !self.caps.enabled {
            return Vec::new();
        }
        let mut out: Vec<&SymDef> = self.defs.values().filter(|d| d.file == file).collect();
        out.sort_by(|a, b| (a.line, &a.name).cmp(&(b.line, &b.name)));
        out
    }

    /// Empties defs + edges, resets counts to zero.
    pub fn clear(&mut self) {
        self.defs.clear();
        self.fwd.clear();
        self.edge_count = 0;
    }

    /// Current `(symbols, edges)` counts.
    #[must_use]
    pub fn stats(&self) -> (usize, usize) {
        if !self.caps.enabled {
            return (0, 0);
        }
        (self.defs.len(), self.edge_count)
    }
}
