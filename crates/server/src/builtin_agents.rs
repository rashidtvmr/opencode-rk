//! Bounded built-in subagent role registry. Original prompt text; the set and
//! policy mirror no third-party catalogue verbatim. `read_only` is an enforced
//! turn policy (tool-list deny), never a prompt suggestion alone.

/// One predefined delegation role. `aliases` normalize common external
/// spellings to the canonical name used by the child session registry.
pub(crate) struct Role {
    pub(crate) name: &'static str,
    pub(crate) aliases: &'static [&'static str],
    pub(crate) description: &'static str,
    pub(crate) prompt: &'static str,
    pub(crate) read_only: bool,
}

const GENERAL: Role = Role {
    name: "general",
    aliases: &["general-purpose"],
    description: "General-purpose subagent with the full tool loop",
    prompt: "Complete the delegated task end to end with an ordinary tool loop. Plan first, execute a bounded number of useful tool calls, verify the result, and report a concise factual summary with exact paths, commands or values. State unverified items explicitly rather than guessing.",
    read_only: false,
};

const EXPLORE: Role = Role {
    name: "explore",
    aliases: &[],
    description: "Explore the codebase and answer structural questions",
    prompt: "Investigate the codebase read-only: locate definitions, call sites and the data flow that answers the question. Cite exact file paths and line anchors. Do not modify anything; report findings with the evidence you inspected.",
    read_only: true,
};

const RESEARCH: Role = Role {
    name: "research",
    aliases: &["researcher"],
    description: "Research web sources and synthesize an answer",
    prompt: "Research the question using available web and documentation tools. Cross-check claims across independent sources, distinguish consensus from a single source, and report findings with source references. Prefer authoritative documentation over blog posts.",
    read_only: true,
};

const DEBUG: Role = Role {
    name: "debug",
    aliases: &["debugger", "root-cause"],
    description: "Reproduce, isolate and diagnose a concrete failure",
    prompt: "Diagnose the reported failure. Form the smallest reproduction, run it, and narrow the cause with targeted evidence. Report the confirmed root cause, the reconciled hypothesis if unconfirmed, and the minimal fix recommendation. Fix only the diagnosed defect.",
    read_only: false,
};

const PLANNER: Role = Role {
    name: "planner",
    aliases: &[],
    description: "Produce a step-by-step implementation plan read-only",
    prompt: "Investigate the relevant code read-only and produce a step-by-step implementation plan: ordered steps, each with the exact files/functions it touches, expected observable outcome, and risks or open questions. Do not implement.",
    read_only: true,
};

const IMPLEMENTER: Role = Role {
    name: "implementer",
    aliases: &["implement", "coder"],
    description: "Implement a specified change with the full tool loop",
    prompt: "Implement exactly the specified change. Reuse existing patterns and dependencies before adding anything, keep the diff minimal and in scope, keep the change compiling and covered by the requested checks, and report what changed and how it was verified.",
    read_only: false,
};

const REVIEWER: Role = Role {
    name: "reviewer",
    aliases: &["review", "code-reviewer"],
    description: "Review specified code or changes read-only",
    prompt: "Review the specified code or diff read-only. Assess correctness, error handling, concurrency and permission/security boundaries against the stated requirements. List findings by severity with exact file locations and a concrete suggested change for each; do not edit files.",
    read_only: true,
};

const TESTER: Role = Role {
    name: "tester",
    aliases: &["test", "testing"],
    description: "Run and diagnose tests, report exact results",
    prompt: "Run exactly the tests, checks or commands requested, capture their real output, and report pass/fail with exact names, commands, durations and failure output. Diagnose a failing assertion to its precise cause. Do not weaken assertions or fix product behavior unless asked.",
    read_only: false,
};

const BUILD: Role = Role {
    name: "build",
    aliases: &[],
    description: "Build or repair code changes (legacy OpenCode-compatible role)",
    prompt: "Build or repair the requested change with the full tool loop as a single bounded work package. Prefer the smallest working diff, keep the build green, and report the exact change, files touched and verification result.",
    read_only: false,
};

const PLAN: Role = Role {
    name: "plan",
    aliases: &[],
    description: "Plan changes read-only (legacy OpenCode-compatible role)",
    prompt: "Produce a read-only plan for the requested change: investigate the affected code, list ordered steps with exact files and functions, expected outcomes, and risks. Do not implement or modify files.",
    read_only: true,
};

/// Fixed catalogue order: the ten canonical roles, aliases collapsed on
/// resolution and never duplicated.
pub(crate) const ROLES: &[Role] =
    &[GENERAL, EXPLORE, RESEARCH, DEBUG, PLANNER, IMPLEMENTER, REVIEWER, TESTER, BUILD, PLAN];

/// Resolve a `subagent_type` spelling safely: exact canonical name, then
/// case-normalized alias, then case-normalized canonical. User-configured
/// agents are checked by the caller first and are never rewritten.
pub(crate) fn resolve(name: &str) -> Option<&'static Role> {
    let candidate = name.trim();
    for role in ROLES.iter() {
        if role.name == candidate { return Some(role); }
    }
    for role in ROLES.iter() {
        for alias in role.aliases {
            if *alias == candidate { return Some(role); }
        }
    }
    let lowered = candidate.to_lowercase();
    for role in ROLES.iter() {
        if role.name == lowered || role.aliases.contains(&lowered.as_str()) { return Some(role); }
    }
    None
}

/// Catalogue entry shape shared with `/api/commands` mention suggestions.
pub(crate) fn catalogue() -> impl Iterator<Item = (&'static str, &'static str, bool)> {
    ROLES.iter().map(|role| (role.name, role.description, role.read_only))
}

#[cfg(test)]
mod role_registry_tests {
    use super::*;

    #[test]
    fn catalogue_has_exactly_ten_unique_canonical_roles() {
        assert_eq!(ROLES.len(), 10);
        let mut names: Vec<_> = ROLES.iter().map(|role| role.name).collect();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), 10);
    }

    #[test]
    fn aliases_resolve_case_insensitively_without_collisions() {
        let mut seen = std::collections::BTreeMap::new();
        for role in ROLES.iter() {
            assert!(seen.insert(role.name, role.name).is_none(), "duplicate canonical {}", role.name);
            for alias in role.aliases {
                assert!(seen.insert(*alias, role.name).is_none(), "alias collision {}", alias);
            }
        }
        for spelling in ROLES.iter()
            .flat_map(|role| std::iter::once(role.name).chain(role.aliases.iter().copied())) {
            assert!(resolve(spelling).is_some(), "unresolvable spelling {}", spelling);
            assert_eq!(resolve(&spelling.to_uppercase()).map(|role| role.name), resolve(spelling).map(|role| role.name));
        }
        // The resolver spec trims surrounding whitespace.
        assert_eq!(resolve("  explore  ").map(|role| role.name), Some("explore"));
        assert!(resolve("nonexistent-role").is_none());
    }

    #[test]
    fn prompts_are_nonempty_bounded_and_distinct() {
        let mut prompts: Vec<&str> = Vec::new();
        for role in ROLES.iter() {
            assert!(!role.prompt.is_empty(), "empty prompt for {}", role.name);
            assert!(role.prompt.len() <= crate::configured_commands::MAX_PROMPT, "unbounded prompt for {}", role.name);
            prompts.push(role.prompt);
        }
        let count = prompts.len();
        prompts.sort_unstable();
        prompts.dedup();
        assert_eq!(prompts.len(), count, "prompts must be distinct");
    }

    #[test]
    fn exactly_five_roles_are_read_only() {
        assert_eq!(ROLES.iter().filter(|role| role.read_only).count(), 5);
    }
}
