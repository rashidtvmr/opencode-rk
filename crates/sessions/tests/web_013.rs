#[path = "../src/web_013.rs"]
mod web_013;

use web_013::{
    Citation, ResearchError, ResearchMode, ResearchRun, RunState, MAX_PROGRESS_EVENTS, MAX_SOURCES,
};

// WEB-013-T01: selected sources yield a reviewable plan, streamed progress,
// steering, and a cited final result.
#[test]
fn web013_t01_research_lifecycle() {
    let mut run = ResearchRun::new(ResearchMode::DeepResearch, true);
    run.set_sources(&["web-search", "docs"]).unwrap();
    run.build_plan(&["survey sources", "draft answer"]).unwrap();
    assert_eq!(run.state(), RunState::Planned);
    assert_eq!(run.plan().len(), 2);
    run.start().unwrap();
    assert_eq!(run.state(), RunState::Running);
    run.push_progress("survey sources", "fetched 2 hits")
        .unwrap();
    run.steer(Some("verify citations")).unwrap();
    assert_eq!(run.state(), RunState::Steered);
    assert_eq!(run.plan().len(), 3);
    run.push_progress("verify citations", "checked quotes")
        .unwrap();
    run.finish(
        2,
        vec![
            Citation {
                claim: 0,
                source: "web-search".to_owned(),
            },
            Citation {
                claim: 1,
                source: "docs".to_owned(),
            },
        ],
        "final with cites",
    )
    .unwrap();
    assert_eq!(run.state(), RunState::Done);
    assert_eq!(run.citations().len(), 2);
}

// WEB-013-T02: without an adapter the mode is explicitly unavailable, and a
// plain chat turn is never mislabeled as research.
#[test]
fn web013_t02_no_adapter_or_plain_chat() {
    let mut run = ResearchRun::new(ResearchMode::Search, false);
    run.set_sources(&["web-search"]).unwrap();
    assert!(matches!(
        run.build_plan(&["step"]),
        Err(ResearchError::Unavailable)
    ));
    assert_ne!(run.state(), RunState::Done);

    assert!(!ResearchRun::is_research_turn(ResearchMode::Chat));
    assert!(ResearchRun::is_research_turn(ResearchMode::Search));
    assert!(ResearchRun::is_research_turn(ResearchMode::DeepResearch));

    let mut run = ResearchRun::new(ResearchMode::Search, true);
    run.set_sources(&["web-search"]).unwrap();
    run.build_plan(&["step"]).unwrap();
    run.start().unwrap();
    let bad = run.finish(
        1,
        vec![Citation {
            claim: 0,
            source: "ghost".to_owned(),
        }],
        "x",
    );
    assert!(matches!(bad, Err(ResearchError::UnknownSource(_))));
    assert_ne!(run.state(), RunState::Done);
    let bad_claim = run.finish(
        2,
        vec![Citation {
            claim: 7,
            source: "web-search".to_owned(),
        }],
        "x",
    );
    assert!(matches!(
        bad_claim,
        Err(ResearchError::ClaimOutOfRange { .. })
    ));
    let uncited = run.finish(
        2,
        vec![Citation {
            claim: 0,
            source: "web-search".to_owned(),
        }],
        "x",
    );
    assert!(matches!(uncited, Err(ResearchError::UncitedClaim(1))));
}

// WEB-013-T03: keyboard-reachable controls per state; polite status carries
// phase state only, never per-event detail spam.
#[test]
fn web013_t03_accessible_keyboard_no_spam() {
    let mut run = ResearchRun::new(ResearchMode::Search, true);
    assert!(run.keyboard_actions().contains(&"choose-mode"));
    assert!(run.keyboard_actions().contains(&"choose-sources"));
    run.set_sources(&["web-search"]).unwrap();
    run.build_plan(&["survey"]).unwrap();
    assert!(run.keyboard_actions().contains(&"review-plan"));
    assert!(run.keyboard_actions().contains(&"start"));
    run.start().unwrap();
    assert!(run.keyboard_actions().contains(&"steer"));
    assert!(run.keyboard_actions().contains(&"cancel"));
    run.push_progress("survey", "token-level detail must not spam live region")
        .unwrap();
    let polite = run.polite_status();
    assert!(!polite.contains("token-level detail"));
    assert!(polite.contains("survey"));
    run.cancel();
    assert!(run.keyboard_actions().contains(&"retry"));
}

// WEB-013-T04: source/progress/child-work bounds hold; cancel reclaims.
#[test]
fn web013_t04_bounds_and_cancel() {
    let mut run = ResearchRun::new(ResearchMode::DeepResearch, true);
    let many: Vec<String> = (0..MAX_SOURCES + 1).map(|i| format!("s{i}")).collect();
    let refs: Vec<&str> = many.iter().map(|s| s.as_str()).collect();
    assert!(matches!(
        run.set_sources(&refs),
        Err(ResearchError::TooManySources { .. })
    ));
    assert!(run.sources().is_empty());

    run.set_sources(&["a"]).unwrap();
    run.build_plan(&["only"]).unwrap();
    run.start().unwrap();
    run.spawn_child().unwrap();
    run.spawn_child().unwrap();
    assert_eq!(run.child_work(), 2);
    for i in 0..(MAX_PROGRESS_EVENTS + 10) {
        run.push_progress("only", &format!("detail-{i}")).unwrap();
    }
    assert!(run.progress_truncated());
    assert_eq!(run.progress().len(), MAX_PROGRESS_EVENTS);

    run.cancel();
    assert_eq!(run.state(), RunState::Cancelled);
    assert_eq!(run.child_work(), 0);
    assert!(run
        .plan()
        .iter()
        .all(|s| s.state != web_013::StepState::Active));
    assert!(run.keyboard_actions().contains(&"retry"));
}

// WEB-013-T05: plan, sources, activity summary and cited result reload.
#[test]
fn web013_t05_replay_from_record() {
    let mut run = ResearchRun::new(ResearchMode::DeepResearch, true);
    run.set_sources(&["web-search", "docs"]).unwrap();
    run.build_plan(&["survey", "draft"]).unwrap();
    run.start().unwrap();
    run.push_progress("survey", "hits").unwrap();
    run.spawn_child().unwrap();
    run.finish(
        1,
        vec![Citation {
            claim: 0,
            source: "docs".to_owned(),
        }],
        "cited final",
    )
    .unwrap();
    let snap = run.snapshot();
    assert_eq!(snap.sources.len(), 2);
    assert_eq!(snap.plan.len(), 2);
    assert_eq!(snap.event_count, 1);
    assert_eq!(snap.citations.len(), 1);
    assert_eq!(snap.final_text, "cited final");

    let restored = ResearchRun::rehydrate(snap.clone()).unwrap();
    assert_eq!(restored.snapshot(), snap);
    assert_eq!(restored.state(), RunState::Done);
}
