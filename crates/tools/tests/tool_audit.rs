use opencode_rk_tools::tool_audit::{MAX_TOOL_AUDIT, ToolAudit, ToolAuditError, record_tool};

#[test]
fn tad_t01_record() {
    let mut buf: Vec<ToolAudit> = Vec::new();
    record_tool(&mut buf, "read", true).expect("record ok");
    assert_eq!(buf.len(), 1);
    assert_eq!(buf[0].tool, "read");
    assert!(buf[0].ok);
    assert_eq!(buf[0].seq, 1);
}

#[test]
fn tad_t02_empty() {
    let mut buf: Vec<ToolAudit> = Vec::new();
    assert!(matches!(
        record_tool(&mut buf, "", false),
        Err(ToolAuditError::EmptyTool)
    ));
    assert!(buf.is_empty());
}

#[test]
fn tad_t03_overflow() {
    let mut buf: Vec<ToolAudit> = Vec::new();
    for i in 0..MAX_TOOL_AUDIT {
        record_tool(&mut buf, &format!("t{i}"), true).expect("fill ok");
    }
    assert_eq!(buf.len(), MAX_TOOL_AUDIT);
    match record_tool(&mut buf, "extra", true) {
        Err(ToolAuditError::TooMany { max, actual }) => {
            assert_eq!(max, MAX_TOOL_AUDIT);
            assert_eq!(actual, MAX_TOOL_AUDIT);
        }
        other => panic!("expected TooMany, got {other:?}"),
    }
    assert_eq!(buf.len(), MAX_TOOL_AUDIT);
}

#[test]
fn tad_t04_order() {
    let mut buf: Vec<ToolAudit> = Vec::new();
    for (name, ok) in [("read", true), ("write", false), ("exec", true)] {
        record_tool(&mut buf, name, ok).expect("record ok");
    }
    let names: Vec<&str> = buf.iter().map(|a| a.tool.as_str()).collect();
    assert_eq!(names, vec!["read", "write", "exec"]);
    assert_eq!(buf[1].ok, false);
}

#[test]
fn tad_t05_seq() {
    let mut buf: Vec<ToolAudit> = Vec::new();
    for name in ["a", "b", "c"] {
        record_tool(&mut buf, name, true).expect("record ok");
    }
    let seqs: Vec<u64> = buf.iter().map(|a| a.seq).collect();
    assert_eq!(seqs, vec![1, 2, 3]);
}
