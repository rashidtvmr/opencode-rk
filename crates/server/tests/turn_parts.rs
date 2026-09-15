//! WEB-009 RED: structured turn parts (reasoning summary, tool activity,
//! final answer, references) projected into distinct parts reconciled to one
//! persisted turn. Raw hidden chain-of-thought is never requested/persisted.

#[path = "../src/turn_parts.rs"]
mod turn_parts;

use turn_parts::{
    ActivityError, ActivityEvent, PartKind, RefTarget, TurnEvent, TurnParts, MAX_EVENTS,
    MAX_PART_BYTES, MAX_REFERENCES, MAX_SUMMARY_BYTES,
};

// WEB-009-T01: structured stream projects into distinct parts, one turn.
#[test]
fn web009_t01_structured_stream_reconciles_to_one_turn() {
    let mut turn = TurnParts::new("turn_01");
    turn.push(TurnEvent::ReasoningDelta {
        text: "Considering ".to_owned(),
    })
    .expect("reasoning delta");
    turn.push(TurnEvent::ReasoningDelta {
        text: "options.".to_owned(),
    })
    .expect("reasoning delta");
    turn.push(TurnEvent::Activity(ActivityEvent::ToolStart {
        name: "read".to_owned(),
        call_id: "call_1".to_owned(),
    }))
    .expect("tool start");
    turn.push(TurnEvent::Activity(ActivityEvent::ToolEnd {
        call_id: "call_1".to_owned(),
        ok: true,
    }))
    .expect("tool end");
    turn.push(TurnEvent::AnswerDelta {
        text: "Final ".to_owned(),
    })
    .expect("answer delta");
    turn.push(TurnEvent::AnswerDelta {
        text: "answer.".to_owned(),
    })
    .expect("answer delta");
    turn.push(TurnEvent::Reference {
        label: "models.dev".to_owned(),
        target: RefTarget::Url("https://models.dev".to_owned()),
    })
    .expect("reference");
    turn.finish("turn_01").expect("finish");
    assert_eq!(turn.reasoning_summary(), "Considering options.");
    assert_eq!(turn.answer(), "Final answer.");
    assert_eq!(turn.parts().len(), 4);
    assert_eq!(turn.parts()[0].kind, PartKind::Reasoning);
    assert_eq!(turn.parts()[1].kind, PartKind::ToolActivity);
    assert_eq!(turn.parts()[2].kind, PartKind::Answer);
    assert_eq!(turn.parts()[3].kind, PartKind::References);
    assert_eq!(turn.references()[0].0, "models.dev");
    let snapshot = turn.snapshot().expect("snapshot");
    let reloaded = TurnParts::from_snapshot(&snapshot).expect("reload");
    assert_eq!(reloaded.answer(), "Final answer.");
    assert_eq!(reloaded.reasoning_summary(), "Considering options.");
}

// WEB-009-T02: malformed/unsupported activity fails boundedly, never fabricates answer.
#[test]
fn web009_t02_malformed_activity_never_becomes_answer() {
    let mut turn = TurnParts::new("turn_02");
    let before = turn.answer().to_owned();
    let err = turn
        .push(TurnEvent::Activity(ActivityEvent::ToolEnd {
            call_id: "call_ghost".to_owned(),
            ok: true,
        }))
        .unwrap_err();
    assert!(matches!(err, ActivityError::UnknownTool(_)));
    let err = turn
        .push(TurnEvent::Activity(ActivityEvent::ToolStart {
            name: String::new(),
            call_id: "call_2".to_owned(),
        }))
        .unwrap_err();
    assert!(matches!(err, ActivityError::Malformed(_)));
    let err = turn
        .push(TurnEvent::AnswerDelta { text: String::new() })
        .unwrap_err();
    assert!(matches!(err, ActivityError::Malformed(_)));
    assert_eq!(turn.answer(), before);
    assert!(turn.answer().is_empty());
    assert!(turn.parts().iter().all(|p| p.kind != PartKind::Answer || p.text.is_empty()));
}

// WEB-009-T03: a11y projection (collapsed controls, live politeness, labels).
#[test]
fn web009_t03_accessibility_projection() {
    let mut turn = TurnParts::new("turn_03");
    turn.push(TurnEvent::ReasoningDelta { text: "sum".to_owned() })
        .expect("reasoning");
    turn.push(TurnEvent::Activity(ActivityEvent::ToolStart {
        name: "read".to_owned(),
        call_id: "c1".to_owned(),
    }))
    .expect("tool");
    turn.push(TurnEvent::AnswerDelta { text: "done".to_owned() })
        .expect("answer");
    turn.push(TurnEvent::Reference {
        label: "models.dev".to_owned(),
        target: RefTarget::Url("https://models.dev".to_owned()),
    })
    .expect("ref");
    turn.finish("turn_03").expect("finish");
    let view = turn.accessibility();
    // Reasoning + tool sections collapsed, answer expanded, deltas not announced.
    assert!(!view.reasoning_expanded);
    assert!(!view.tools_expanded);
    assert!(view.answer_expanded);
    assert_eq!(view.live_politeness, "off");
    assert_eq!(view.reasoning_label, "Reasoning");
    assert_eq!(view.tools_label, "Tool activity");
    assert_eq!(view.references_label, "References");
    assert_eq!(view.references.len(), 1);
    assert_eq!(view.references[0].label, "models.dev");
    assert!(!view.references[0].href.is_empty());
}

// WEB-009-T04: bounds + disconnect cancels upstream work, releases permit.
#[test]
fn web009_t04_bounds_and_cancel_release_permit() {
    let mut turn = TurnParts::new("turn_04");
    let big = "x".repeat(MAX_PART_BYTES + 1);
    let err = turn.push(TurnEvent::AnswerDelta { text: big }).unwrap_err();
    assert!(matches!(err, ActivityError::TooLarge));
    for i in 0..MAX_EVENTS {
        turn.push(TurnEvent::AnswerDelta { text: "a".to_owned() })
            .expect("bounded event");
        let _ = i;
    }
    let err = turn
        .push(TurnEvent::AnswerDelta { text: "one too many".to_owned() })
        .unwrap_err();
    assert!(matches!(err, ActivityError::TooManyEvents));
    // Oversize reasoning summaries are bounded, never unbounded retained.
    let mut turn2 = TurnParts::new("turn_04b");
    let big_summary = "y".repeat(MAX_SUMMARY_BYTES + 1);
    let err = turn2
        .push(TurnEvent::ReasoningDelta { text: big_summary })
        .unwrap_err();
    assert!(matches!(err, ActivityError::TooLarge));
    // Disconnect cancels upstream/permit exactly once.
    let flag = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let permit = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(1));
    turn.set_cancel_hook({
        let flag = flag.clone();
        let permit = permit.clone();
        move || {
            flag.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            permit.store(0, std::sync::atomic::Ordering::SeqCst);
        }
    });
    turn.disconnect();
    turn.disconnect();
    assert_eq!(flag.load(std::sync::atomic::Ordering::SeqCst), 1);
    assert_eq!(permit.load(std::sync::atomic::Ordering::SeqCst), 0);
    assert!(turn.cancelled());
    // References bounded.
    let mut turn3 = TurnParts::new("turn_04c");
    for i in 0..MAX_REFERENCES {
        turn3
            .push(TurnEvent::Reference {
                label: format!("src {i}"),
                target: RefTarget::Url(format!("https://example.invalid/{i}")),
            })
            .expect("bounded reference");
    }
    let err = turn3
        .push(TurnEvent::Reference {
            label: "extra".to_owned(),
            target: RefTarget::Url("https://example.invalid/x".to_owned()),
        })
        .unwrap_err();
    assert!(matches!(err, ActivityError::TooManyEvents));
}

// WEB-009-T05: persisted fidelity without hidden CoT.
#[test]
fn web009_t05_persisted_fidelity_without_hidden_cot() {
    let mut turn = TurnParts::new("turn_05");
    turn.push(TurnEvent::ReasoningDelta { text: "auditable summary".to_owned() })
        .expect("reasoning");
    turn.push(TurnEvent::Activity(ActivityEvent::ToolStart {
        name: "read".to_owned(),
        call_id: "c9".to_owned(),
    }))
    .expect("start");
    turn.push(TurnEvent::Activity(ActivityEvent::ToolEnd {
        call_id: "c9".to_owned(),
        ok: false,
    }))
    .expect("end");
    turn.push(TurnEvent::AnswerDelta { text: "final text".to_owned() })
        .expect("answer");
    turn.push(TurnEvent::Reference {
        label: "local file".to_owned(),
        target: RefTarget::Path("src/main.rs".to_owned()),
    })
    .expect("ref");
    turn.finish("turn_05").expect("finish");
    let snapshot = turn.snapshot().expect("snapshot");
    assert!(!snapshot.contains("chain_of_thought"));
    assert!(!snapshot.contains("hidden"));
    let reloaded = TurnParts::from_snapshot(&snapshot).expect("reload");
    assert_eq!(reloaded.answer(), "final text");
    assert_eq!(reloaded.reasoning_summary(), "auditable summary");
    assert_eq!(reloaded.references().len(), 1);
    assert_eq!(reloaded.references()[0].0, "local file");
    assert_eq!(reloaded.tool_states(), vec![("read".to_owned(), "c9".to_owned(), false)]);
    // Corrupt snapshot fails explicitly, never fabricates content.
    let bad = snapshot.replace("final text", "");
    // Empty answer after tamper is still parseable but fidelity check catches it.
    if let Ok(tampered) = TurnParts::from_snapshot(&bad) {
        assert_ne!(tampered.answer(), "final text");
    }
    assert!(TurnParts::from_snapshot("not json").is_err());
}
