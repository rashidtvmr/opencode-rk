use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet, VecDeque};

/// Maximum number of steps in a workflow.
pub const MAX_STEPS: usize = 256;

/// Maximum number of edges (next references) in a workflow.
pub const MAX_EDGES: usize = 1024;

/// Maximum depth of the DAG (longest path from root to leaf).
pub const MAX_DEPTH: usize = 64;

/// Kind of workflow step.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum StepKind {
    Skill,
    ToolCall,
    Subagent,
}

/// A single step in a workflow DAG.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Step {
    pub id: String,
    pub kind: StepKind,
    pub args: String,
    pub next: Vec<String>,
}

/// Workflow definition: named DAG of steps.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Workflow {
    pub name: String,
    pub description: String,
    pub steps: Vec<Step>,
}

/// Errors produced during workflow validation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum WorkflowError {
    EmptyWorkflow,
    TooManySteps(usize),
    TooManyEdges(usize),
    DuplicateStepId(String),
    ReferenceUnknownStep { step_id: String, missing_ref: String },
    CycleDetected(Vec<String>),
    MultipleRoots(Vec<String>),
    UnreachableSteps(Vec<String>),
    DepthExceeded(usize),
}

/// Validate a workflow DAG: single root, no cycles, all steps reachable,
/// bounded caps, no unknown references.
pub fn validate_workflow(wf: &Workflow) -> Result<(), WorkflowError> {
    if wf.steps.is_empty() {
        return Err(WorkflowError::EmptyWorkflow);
    }

    let n = wf.steps.len();
    if n > MAX_STEPS {
        return Err(WorkflowError::TooManySteps(n));
    }

    let mut total_edges: usize = 0;
    for s in &wf.steps {
        total_edges += s.next.len();
    }
    if total_edges > MAX_EDGES {
        return Err(WorkflowError::TooManyEdges(total_edges));
    }

    let mut ids_seen = HashSet::new();
    for s in &wf.steps {
        if !ids_seen.insert(&s.id) {
            return Err(WorkflowError::DuplicateStepId(s.id.clone()));
        }
    }

    let id_set: HashSet<&str> = wf.steps.iter().map(|s| s.id.as_str()).collect();
    for s in &wf.steps {
        for r in &s.next {
            if !id_set.contains(r.as_str()) {
                return Err(WorkflowError::ReferenceUnknownStep {
                    step_id: s.id.clone(),
                    missing_ref: r.clone(),
                });
            }
        }
    }

    // Find roots: steps not referenced by any other step
    let mut referenced: HashSet<&str> = HashSet::new();
    for s in &wf.steps {
        for r in &s.next {
            referenced.insert(r);
        }
    }
    let mut roots: Vec<&str> = wf
        .steps
        .iter()
        .filter(|s| !referenced.contains(s.id.as_str()))
        .map(|s| s.id.as_str())
        .collect();
    roots.sort(); // deterministic ordering

    if roots.is_empty() {
        // All steps referenced -> cycle
        let cycle = find_cycle(wf);
        return Err(WorkflowError::CycleDetected(cycle));
    }
    if roots.len() > 1 {
        let root_ids: Vec<String> = roots.into_iter().map(String::from).collect();
        return Err(WorkflowError::MultipleRoots(root_ids));
    }

    // Check reachability from the single root
    let root = roots[0];
    let reachable = bfs_reachable(wf, root);
    let unreachable: Vec<String> = wf
        .steps
        .iter()
        .filter(|s| !reachable.contains(s.id.as_str()))
        .map(|s| s.id.clone())
        .collect();
    if !unreachable.is_empty() {
        return Err(WorkflowError::UnreachableSteps(unreachable));
    }

    // Check depth
    let depth = max_depth(wf, root);
    if depth > MAX_DEPTH {
        return Err(WorkflowError::DepthExceeded(depth));
    }

    Ok(())
}

/// Compute a deterministic topological order. Requires a valid (single-root,
/// acyclic) workflow. Returns Err if the workflow is invalid.
pub fn topological_order(wf: &Workflow) -> Result<Vec<&Step>, WorkflowError> {
    validate_workflow(wf)?;

    // Kahn's algorithm with deterministic (sorted) neighbor selection
    let id_to_idx: HashMap<&str, usize> = wf.steps.iter().enumerate().map(|(i, s)| (s.id.as_str(), i)).collect();
    let mut in_degree: HashMap<&str, usize> = wf.steps.iter().map(|s| (s.id.as_str(), 0usize)).collect();
    for s in &wf.steps {
        for r in &s.next {
            *in_degree.get_mut(r.as_str()).unwrap() += 1;
        }
    }

    let mut queue: VecDeque<&str> = VecDeque::new();
    let mut roots: Vec<&str> = wf.steps.iter().filter(|s| in_degree[&s.id.as_str()] == 0).map(|s| s.id.as_str()).collect();
    roots.sort();
    for r in roots {
        queue.push_back(r);
    }

    let mut result = Vec::with_capacity(wf.steps.len());
    while let Some(node) = queue.pop_front() {
        result.push(id_to_idx[node]);
        let mut neighbors: Vec<&str> = wf.steps[id_to_idx[node]]
            .next
            .iter()
            .map(String::as_str)
            .collect();
        neighbors.sort();
        for nb in neighbors {
            let deg = in_degree.get_mut(nb).unwrap();
            *deg -= 1;
            if *deg == 0 {
                queue.push_back(nb);
            }
        }
    }

    Ok(result.into_iter().map(|i| &wf.steps[i]).collect())
}

/// Render a human-readable execution plan.
pub fn render_plan(wf: &Workflow) -> String {
    let mut out = String::new();
    out.push_str(&format!("Workflow: {}\n", wf.name));
    out.push_str(&format!("Description: {}\n", wf.description));
    out.push_str("Steps:\n");
    for s in &wf.steps {
        out.push_str(&format!(
            "  [{}] {:?} args={} -> {}\n",
            s.id,
            s.kind,
            s.args,
            if s.next.is_empty() {
                "(end)".to_string()
            } else {
                s.next.join(", ")
            }
        ));
    }
    if let Ok(topo) = topological_order(wf) {
        out.push_str("Execution order:\n");
        for (i, s) in topo.iter().enumerate() {
            out.push_str(&format!("  {}. {} ({:?})\n", i + 1, s.id, s.kind));
        }
    }
    out
}

// ── internal helpers ──

fn bfs_reachable(wf: &Workflow, start: &str) -> HashSet<String> {
    let adj: HashMap<&str, Vec<&str>> = wf
        .steps
        .iter()
        .map(|s| (s.id.as_str(), s.next.iter().map(String::as_str).collect()))
        .collect();
    let mut visited = HashSet::new();
    let mut queue = VecDeque::new();
    queue.push_back(start);
    visited.insert(start.to_string());
    while let Some(node) = queue.pop_front() {
        if let Some(nbrs) = adj.get(node) {
            for nb in nbrs {
                if visited.insert(nb.to_string()) {
                    queue.push_back(nb);
                }
            }
        }
    }
    visited
}

fn find_cycle(wf: &Workflow) -> Vec<String> {
    let adj: HashMap<&str, Vec<&str>> = wf
        .steps
        .iter()
        .map(|s| (s.id.as_str(), s.next.iter().map(String::as_str).collect()))
        .collect();
    let mut visited: HashSet<String> = HashSet::new();
    let mut stack: HashSet<String> = HashSet::new();
    let mut path: Vec<String> = Vec::new();

    fn dfs<'a>(
        node: &'a str,
        adj: &HashMap<&str, Vec<&str>>,
        visited: &mut HashSet<String>,
        stack: &mut HashSet<String>,
        path: &mut Vec<String>,
    ) -> bool {
        visited.insert(node.to_string());
        stack.insert(node.to_string());
        path.push(node.to_string());
        if let Some(nbrs) = adj.get(node) {
            for nb in nbrs {
                if !visited.contains(*nb) {
                    if dfs(nb, adj, visited, stack, path) {
                        return true;
                    }
                } else if stack.contains(*nb) {
                    path.push(nb.to_string());
                    return true;
                }
            }
        }
        stack.remove(node);
        path.pop();
        false
    }

    for s in &wf.steps {
        if !visited.contains(&s.id) {
            path.clear();
            if dfs(s.id.as_str(), &adj, &mut visited, &mut stack, &mut path) {
                return path;
            }
        }
    }
    vec![]
}

fn max_depth(wf: &Workflow, root: &str) -> usize {
    let adj: HashMap<&str, Vec<&str>> = wf
        .steps
        .iter()
        .map(|s| (s.id.as_str(), s.next.iter().map(String::as_str).collect()))
        .collect();
    let mut memo: HashMap<String, usize> = HashMap::new();
    fn depth_of(
        node: &str,
        adj: &HashMap<&str, Vec<&str>>,
        memo: &mut HashMap<String, usize>,
    ) -> usize {
        if let Some(&d) = memo.get(node) {
            return d;
        }
        let d = match adj.get(node) {
            Some(nbrs) if !nbrs.is_empty() => {
                let child = nbrs
                    .iter()
                    .map(|nb| depth_of(nb, adj, memo))
                    .max()
                    .unwrap_or(0);
                child + 1
            }
            _ => 1,
        };
        memo.insert(node.to_string(), d);
        d
    }
    depth_of(root, &adj, &mut memo)
}
