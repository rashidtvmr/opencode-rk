//! Integration tests for the command_templates module.
//!
//! Uses the standalone `#[path]` include pattern (same as agent_files.rs).
//! Each test is labeled T01..T14. Frozen RED hash recorded before GREEN.

#[path = "../src/command_templates.rs"]
mod command_templates;

use command_templates::{
    expand_template, parse_command_def, round_trip, serialize_command_def, CommandDef, CommandError,
    FileRef, ShellPlan, MAX_FILE_REFS, MAX_POSITIONAL_ARGS, MAX_TEMPLATE_BYTES,
};

// ── T01: basic $ARGUMENTS substitution ─────────────────────────────────────

#[test]
fn t01_arguments_substitution() {
    let def = CommandDef {
        name: "greet".into(),
        description: "".into(),
        template: "Hello $ARGUMENTS!".into(),
    };
    let result = expand_template(&def, &["world".into()]).unwrap();
    assert_eq!(result.text, "Hello world!");
    assert!(result.file_refs.is_empty());
    assert!(result.shell_plan.is_none());
}

// ── T02: positional $1 $2 substitution ────────────────────────────────────

#[test]
fn t02_positional_substitution() {
    let def = CommandDef {
        name: "fmt".into(),
        description: "".into(),
        template: "format $1 with $2".into(),
    };
    let result = expand_template(&def, &["file.rs".into(), "rustfmt".into()]).unwrap();
    assert_eq!(result.text, "format file.rs with rustfmt");
}

// ── T03: $ARGUMENTS with no args → empty string ────────────────────────────

#[test]
fn t03_arguments_empty() {
    let def = CommandDef {
        name: "noop".into(),
        description: "".into(),
        template: "args=[$ARGUMENTS]".into(),
    };
    let result = expand_template(&def, &[]).unwrap();
    assert_eq!(result.text, "args=[]");
}

// ── T04: @file mention → FileRef list ──────────────────────────────────────

#[test]
fn t04_file_ref_extraction() {
    let def = CommandDef {
        name: "review".into(),
        description: "".into(),
        template: "Review @src/main.rs and @lib/mod.rs".into(),
    };
    let result = expand_template(&def, &[]).unwrap();
    assert_eq!(
        result.file_refs,
        vec![
            FileRef {
                path: "src/main.rs".into()
            },
            FileRef {
                path: "lib/mod.rs".into()
            }
        ]
    );
    // File refs are also preserved in text
    assert_eq!(result.text, "Review @src/main.rs and @lib/mod.rs");
}

// ── T05: @file traversal rejection ─────────────────────────────────────────

#[test]
fn t05_file_ref_traversal_rejected() {
    let def = CommandDef {
        name: "bad".into(),
        description: "".into(),
        template: "Read @../../etc/passwd".into(),
    };
    let err = expand_template(&def, &[]).unwrap_err();
    assert!(
        matches!(err, CommandError::InvalidFilePath(_)),
        "traversal must be rejected, got: {err}"
    );
}

// ── T06: @file absolute path rejection ─────────────────────────────────────

#[test]
fn t06_file_ref_absolute_rejected() {
    let def = CommandDef {
        name: "bad".into(),
        description: "".into(),
        template: "Read @/etc/passwd".into(),
    };
    let err = expand_template(&def, &[]).unwrap_err();
    assert!(
        matches!(err, CommandError::InvalidFilePath(_)),
        "absolute path must be rejected, got: {err}"
    );
}

// ── T07: !shell prefix → ShellPlan without execution ───────────────────────

#[test]
fn t07_shell_plan_not_executed() {
    let def = CommandDef {
        name: "run".into(),
        description: "".into(),
        template: "!shell cargo test".into(),
    };
    let result = expand_template(&def, &[]).unwrap();
    assert_eq!(
        result.shell_plan,
        Some(ShellPlan {
            command: "cargo test".into(),
            requires_approval: true,
        })
    );
    // Text retains the raw template (broker strips the prefix when dispatching)
    assert_eq!(result.text, "!shell cargo test");
}

// ── T08: unknown template variable → error ─────────────────────────────────

#[test]
fn t08_unknown_variable_error() {
    let def = CommandDef {
        name: "bad".into(),
        description: "".into(),
        template: "Hello $UNKNOWN!".into(),
    };
    let err = expand_template(&def, &[]).unwrap_err();
    assert!(
        matches!(err, CommandError::UnknownVariable(ref v) if v == "UNKNOWN"),
        "unknown variable must error, got: {err}"
    );
}

// ── T09: bounded template size ──────────────────────────────────────────────

#[test]
fn t09_template_size_bound() {
    let raw = format!(
        "---\nname: big\n---\n{}",
        "x".repeat(MAX_TEMPLATE_BYTES + 1)
    );
    let err = parse_command_def(&raw).unwrap_err();
    assert!(
        matches!(err, CommandError::TemplateTooLarge { .. }),
        "oversized template must fail, got: {err}"
    );
}

// ── T10: bounded argument count ─────────────────────────────────────────────

#[test]
fn t10_argument_count_bound() {
    let def = CommandDef {
        name: "many".into(),
        description: "".into(),
        template: "$ARGUMENTS".into(),
    };
    let args: Vec<String> = (0..=MAX_POSITIONAL_ARGS).map(|i| format!("a{i}")).collect();
    let err = expand_template(&def, &args).unwrap_err();
    assert!(
        matches!(err, CommandError::TooManyPositionalArgs { .. }),
        "too many args must error, got: {err}"
    );
}

// ── T11: bounded file-ref count ────────────────────────────────────────────

#[test]
fn t11_file_ref_count_bound() {
    let refs: Vec<String> = (0..=MAX_FILE_REFS)
        .map(|i| format!("@file{i}.txt"))
        .collect();
    let template = refs.join(" ");
    let def = CommandDef {
        name: "many".into(),
        description: "".into(),
        template,
    };
    let err = expand_template(&def, &[]).unwrap_err();
    assert!(
        matches!(err, CommandError::TooManyFileRefs { .. }),
        "too many file refs must error, got: {err}"
    );
}

// ── T12: determinism — same inputs → same output ───────────────────────────

#[test]
fn t12_determinism() {
    let def = CommandDef {
        name: "det".into(),
        description: "".into(),
        template: "$1 + $ARGUMENTS = $2".into(),
    };
    let args = vec!["a".into(), "b".into()];
    let r1 = expand_template(&def, &args).unwrap();
    let r2 = expand_template(&def, &args).unwrap();
    assert_eq!(r1.text, r2.text);
    assert_eq!(r1.file_refs, r2.file_refs);
    assert_eq!(r1.shell_plan, r2.shell_plan);
}

// ── T13: round-trip serialize → parse ──────────────────────────────────────

#[test]
fn t13_round_trip() {
    let def = CommandDef {
        name: "my-cmd".into(),
        description: "A test command".into(),
        template: "Hello $ARGUMENTS, ref @file.rs".into(),
    };
    let rt = round_trip(&def).unwrap();
    assert_eq!(rt.name, def.name);
    assert_eq!(rt.description, def.description);
    assert_eq!(rt.template, def.template);
}

// ── T14: invalid name → error ──────────────────────────────────────────────

#[test]
fn t14_invalid_name_rejected() {
    let raw = "---\nname: bad name with spaces\n---\nHello";
    let err = parse_command_def(raw).unwrap_err();
    assert!(
        matches!(err, CommandError::InvalidName(_)),
        "invalid name must error, got: {err}"
    );
}
