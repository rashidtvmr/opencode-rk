//! Failure-only output filters for test/build/lint commands (TOOL-020).
//!
//! Pure presentational transform over a captured log: all-pass runs keep
//! the verdict line(s) only; failing runs keep every failing-test/rule
//! block (header plus up to [`CTX_LINES`] trailing context lines each)
//! plus the original verdict line(s) verbatim. The exit code is never
//! altered and a failing run never renders as green. No I/O, no clock,
//! no network, no global state beyond the documented `RTK_NO_FILTER`
//! opt-out environment read.

/// Supported tools.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TestTool {
    Pytest,
    CargoTest,
    Tsc,
    Lint,
}

/// Retained-output budget per run (default).
pub const MAX_KEPT_BYTES: usize = 64 * 1024;

/// Trailing context lines kept per failure block.
pub const CTX_LINES: usize = 30;

/// Max failure blocks kept; the rest are counted and noted.
pub const MAX_BLOCKS: usize = 20;

/// Tail lines emitted when a failing run matches no failure block.
pub const TAIL_LINES: usize = 50;

/// Filter options.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FilterOpts {
    /// Opt out: return input verbatim, no parse.
    pub no_filter: bool,
    /// Byte budget for retained output.
    pub max_kept_bytes: usize,
    /// Path named in the truncation marker.
    pub log_path: String,
}

impl Default for FilterOpts {
    fn default() -> Self {
        Self {
            no_filter: false,
            max_kept_bytes: MAX_KEPT_BYTES,
            log_path: "<stdout>".to_string(),
        }
    }
}

/// Filter result. `exit` always equals the input exit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FilteredOutput {
    pub text: String,
    pub verdict: String,
    pub exit: i32,
    pub truncated: bool,
}

fn is_verdict(tool: TestTool, line: &str) -> bool {
    let t = line.trim();
    match tool {
        TestTool::Pytest => {
            (t.contains("passed") || t.contains("failed") || t.contains("error"))
                && (t.contains(" in ") || t.starts_with('='))
        }
        TestTool::CargoTest => t.contains("test result:"),
        TestTool::Tsc => t.starts_with("Found ") && t.contains("error"),
        TestTool::Lint => t.contains("problem"),
    }
}

fn is_failure_header(tool: TestTool, line: &str) -> bool {
    if is_verdict(tool, line) {
        return false;
    }
    let t = line.trim();
    match tool {
        TestTool::Pytest => t.contains("FAILED") || t.starts_with("ERROR "),
        TestTool::CargoTest => {
            (t.starts_with("test ") && t.contains("FAILED"))
                || t == "failures:"
                || t.ends_with("stdout ----")
        }
        TestTool::Tsc => t.contains("error TS"),
        TestTool::Lint => {
            t.starts_with(|c: char| c.is_ascii_digit())
                && (t.contains("error") || t.contains("warning"))
                || t.contains(" error ")
                || t.contains(" warning ")
        }
    }
}

fn truncation_marker(log_path: &str) -> String {
    format!("... [rtk: truncated, full log in {log_path}]")
}

/// Filter `stdout` for `tool`, preserving `exit` verbatim.
pub fn filter_test_output(
    tool: TestTool,
    stdout: &str,
    exit: i32,
    opts: &FilterOpts,
) -> FilteredOutput {
    if opts.no_filter || std::env::var("RTK_NO_FILTER").as_deref() == Ok("1") {
        return FilteredOutput {
            text: stdout.to_string(),
            verdict: String::new(),
            exit,
            truncated: false,
        };
    }

    let lines: Vec<&str> = stdout.lines().collect();
    let mut verdicts: Vec<&str> = Vec::new();
    let mut blocks: Vec<Vec<&str>> = Vec::new();
    let mut omitted: usize = 0;
    let mut ctx_left: usize = 0;

    for line in &lines {
        if is_verdict(tool, line) {
            verdicts.push(line);
            ctx_left = 0;
            continue;
        }
        if is_failure_header(tool, line) {
            if blocks.len() < MAX_BLOCKS {
                blocks.push(vec![line]);
                ctx_left = CTX_LINES;
            } else {
                omitted += 1;
                ctx_left = 0;
            }
            continue;
        }
        if ctx_left > 0 {
            if let Some(last) = blocks.last_mut() {
                last.push(line);
            }
            ctx_left -= 1;
        }
    }

    let verdict = verdicts.join("\n");
    let budget = opts.max_kept_bytes.max(1024);

    if exit == 0 {
        if verdicts.is_empty() {
            let line = "ok (exit 0)".to_string();
            return FilteredOutput {
                text: line.clone(),
                verdict: line,
                exit,
                truncated: false,
            };
        }
        return FilteredOutput {
            text: verdict.clone(),
            verdict,
            exit,
            truncated: false,
        };
    }

    // Failure path: never emit empty output, never look green.
    let mut kept: Vec<&str> = Vec::new();
    for b in &blocks {
        kept.extend(b.iter().copied());
    }
    for v in &verdicts {
        if !kept.iter().any(|k| *k == *v) {
            kept.push(v);
        }
    }
    if kept.is_empty() {
        let tail: Vec<&str> = lines
            .iter()
            .copied()
            .rev()
            .take(TAIL_LINES)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect();
        if tail.is_empty() {
            let line = format!("command failed (exit {exit}), no output captured");
            return FilteredOutput {
                text: line.clone(),
                verdict: line,
                exit,
                truncated: false,
            };
        }
        let text = tail.join("\n");
        let tail_verdict = if verdict.is_empty() {
            text.clone()
        } else {
            verdict.clone()
        };
        return FilteredOutput {
            text,
            verdict: tail_verdict,
            exit,
            truncated: false,
        };
    }

    let mut text = kept.join("\n");
    let truncated = omitted > 0;
    if truncated {
        text.push_str(&format!("\n... [rtk: {omitted} more failure(s) omitted]"));
        text.push_str(&format!("\n{}", truncation_marker(&opts.log_path)));
    }
    if !truncated && text.len() <= budget {
        return FilteredOutput {
            text,
            verdict,
            exit,
            truncated: false,
        };
    }
    if truncated && text.len() <= budget {
        return FilteredOutput {
            text,
            verdict,
            exit,
            truncated: true,
        };
    }

    // Over budget: keep first failing block + verdict + marker.
    let marker = truncation_marker(&opts.log_path);
    let mut first: Vec<&str> = blocks
        .first()
        .map(|b| b.iter().copied().collect())
        .unwrap_or_default();
    let mut head = first.join("\n");
    let tail_reserve = verdict.len() + marker.len() + 2;
    let mut head_budget = budget.saturating_sub(tail_reserve);
    while head.len() > head_budget && first.len() > 1 {
        first.pop();
        head = first.join("\n");
    }
    if head.len() > head_budget {
        head_budget = head_budget.min(head.len());
        let mut cut = head_budget;
        while cut > 0 && !head.is_char_boundary(cut) {
            cut -= 1;
        }
        head.truncate(cut);
    }
    let mut out = head;
    if !verdict.is_empty() {
        out.push('\n');
        out.push_str(&verdict);
    }
    out.push('\n');
    out.push_str(&marker);
    FilteredOutput {
        text: out,
        verdict,
        exit,
        truncated: true,
    }
}
