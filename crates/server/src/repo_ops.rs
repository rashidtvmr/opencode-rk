//! Pure git-operation planner: validate + plan, no execution, no IO.

/// Maximum number of args a plan may carry.
pub const MAX_PLAN_ARGS: usize = 16;

/// Git operations supported by the planner.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RepoOp {
    Status,
    Fetch,
    Pull,
    Push,
    Commit { message: String },
}

/// Validated plan: git subcommand name + argv (without `git` prefix).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepoPlan {
    pub op: String,
    pub args: Vec<String>,
    pub read_only: bool,
}

/// Planner failures.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RepoOpsError {
    EmptyPath,
    EmptyMessage,
    UnsupportedOp,
    TooManyArgs { max: usize, actual: usize },
}

impl std::fmt::Display for RepoOpsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EmptyPath => write!(f, "empty repo path"),
            Self::EmptyMessage => write!(f, "empty commit message"),
            Self::UnsupportedOp => write!(f, "unsupported operation"),
            Self::TooManyArgs { max, actual } => {
                write!(f, "too many args: max {max}, got {actual}")
            }
        }
    }
}

impl std::error::Error for RepoOpsError {}

/// Validate inputs and build a deterministic git plan. No execution, no IO.
pub fn plan_repo_op(repo_path: &str, op: &RepoOp) -> Result<RepoPlan, RepoOpsError> {
    if repo_path.is_empty() {
        return Err(RepoOpsError::EmptyPath);
    }
    let (op_name, args, read_only) = match op {
        RepoOp::Status => (
            "status",
            vec![
                "status".to_string(),
                "--short".to_string(),
                "--branch".to_string(),
            ],
            true,
        ),
        RepoOp::Fetch => (
            "fetch",
            vec!["fetch".to_string(), "--prune".to_string()],
            true,
        ),
        RepoOp::Pull => (
            "pull",
            vec!["pull".to_string(), "--ff-only".to_string()],
            false,
        ),
        RepoOp::Push => ("push", vec!["push".to_string()], false),
        RepoOp::Commit { message } => {
            if message.trim().is_empty() {
                return Err(RepoOpsError::EmptyMessage);
            }
            let truncated = if message.chars().count() > 500 {
                let head: String = message.chars().take(500).collect();
                format!("{head}...")
            } else {
                message.clone()
            };
            (
                "commit",
                vec!["commit".to_string(), "-m".to_string(), truncated],
                false,
            )
        }
    };
    if args.len() > MAX_PLAN_ARGS {
        return Err(RepoOpsError::TooManyArgs {
            max: MAX_PLAN_ARGS,
            actual: args.len(),
        });
    }
    Ok(RepoPlan {
        op: op_name.to_string(),
        args,
        read_only,
    })
}
