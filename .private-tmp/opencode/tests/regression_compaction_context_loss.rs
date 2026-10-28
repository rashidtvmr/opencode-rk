//! UNEXECUTED regression: held background summary lost on owner cancellation.
//! See AGENTS.md Phase-1 acceptance (installed runner owns execution).
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;
use tempfile::tempdir;
use tokio::sync::Notify;

#[tokio::test]
async fn held_summary_dropped_after_owner_cancel_still_retries() {
    let _ = ENV_LOCK.lock().await;
    let root = tempdir().unwrap();
    let state = ProviderState {
        calls: Arc::new(AtomicUsize::new(0)),
        summaries: Arc::new(AtomicUsize::new(0)),
        summary_seen: Arc::new(Notify::new()),
        release_first_summary: Arc::new(Notify::new()),
        block_first_summary: Arc::new(std::sync::atomic::AtomicBool::new(true)),
        usage_responses: Arc::new(AtomicUsize::new(0)),
        partial_seen: Arc::new(Notify::new()),
        normal_input: Arc::new(tokio::sync::Mutex::new(Vec::new())),
    };
    let (base, pt) = provider(state.clone()).await;
    let _ = Environment::set(&[
        ("OPENAI_BASE_URL", base),
        ("OPENAI_API_KEY", "disposable-regression".into()),
        ("OPENCODE_RK_HOME", root.path().display().to_string()),
        ("OPENCODE_RK_COMPACTION__ENABLED", "true".into()),
        ("OPENCODE_RK_COMPACTION__STRATEGY", "fallback".into()),
        ("OPENCODE_RK_COMPACTION__THRESHOLD_TOKENS", "140000".into()),
        ("OPENCODE_RK_COMPACTION__KEEP_RECENT_TURNS", "1".into()),
    ]);
    let sessions = SessionService::new(Arc::new(
        Storage::open_in_memory(root.path().join("blobs")).unwrap(),
    ));
    let session = sessions
        .create_in_directory("regression ctx loss", Some(root.path().to_owned()))
        .await
        .unwrap();
    for i in 0..6 {
        sessions
            .append_text(session.id, MessageRole::User, format!("u{i} {}", "x".repeat(48_000)))
            .await
            .unwrap();
        sessions
            .append_text(session.id, MessageRole::Assistant, format!("a{i} {}", "y".repeat(48_000)))
            .await
            .unwrap();
    }
    let _ = post_turn(
        router(AppState {
            sessions: sessions.clone(),
            catalog: Arc::new(Catalog::default()),
        }),
        session.id,
        "trigger".into(),
    )
    .await;
    tokio::time::timeout(Duration::from_secs(2), state.summary_seen.notified())
        .await
        .expect("summary must be requested");
    state.release_first_summary.notify_waiters();
    let before = state.summaries.load(Ordering::SeqCst);
    let _ = post_turn(
        router(AppState {
            sessions: sessions.clone(),
            catalog: Arc::new(Catalog::default()),
        }),
        session.id,
        "z".repeat(48_000).into(),
    )
    .await;
    let after = state.summaries.load(Ordering::SeqCst);
    assert!(
        after > before,
        "fallback must invoke a second summary when held context is lost after owner cancel"
    );
    pt.shutdown().await;
}
