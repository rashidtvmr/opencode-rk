
//! Native or tool-free summary compaction at the turn boundary.
//!
//! This is deliberately a small boundary policy, rather than a second history
//! implementation.  The session service remains the source of truth for
//! ordering and bounds; this module only decides whether to call the provider.

use super::*;
use opencode_rk_providers::responses::ReportedUsage;
use std::collections::HashMap;
use std::sync::Mutex;

/// A completed background summary is held in memory and is never persisted by
/// this module. The foreground compaction path consumes it only after checking
/// that its conversation prefix, model, and retained tail still match.
static BACKGROUND: std::sync::OnceLock<Mutex<HashMap<SessionId, HeldBackground>>> =
    std::sync::OnceLock::new();

const BACKGROUND_RATIO: u64 = 85;
/// Minimum verbatim tail accepted beside a precomputed summary, matching the
/// upstream PRECOMPUTED_TAIL_MIN guard.
const PRECOMPUTED_TAIL_MIN: u64 = 30_000;

/// Identity of the actual provider-context prefix and model variant used by a
/// turn.  Ordinary HTTP turns have no injected instruction prefix, so their
/// effective initial value is intentionally empty.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct BackgroundRequestContext {
    pub initial: String,
    pub variant: Option<String>,
}

pub(crate) fn request_context(
    agent_prompt: Option<&str>,
    reasoning_effort: &str,
) -> BackgroundRequestContext {
    BackgroundRequestContext {
        initial: agent_prompt.unwrap_or_default().to_owned(),
        variant: (!reasoning_effort.is_empty()).then(|| reasoning_effort.to_owned()),
    }
}

fn precomputed_tail_limit(budget_tokens: u64) -> u64 {
    PRECOMPUTED_TAIL_MIN.min(budget_tokens.saturating_mul(25) / 100)
}

fn retained_tail_tokens(messages: &[MessageRecord], anchor_index: usize) -> u64 {
    let tail_bytes = messages
        .iter()
        .skip(anchor_index)
        .map(|message| match &message.body {
            opencode_rk_contracts::PayloadRef::Inline { text } => text.len(),
            opencode_rk_contracts::PayloadRef::Blob { bytes, .. } => {
                (*bytes).min(usize::MAX as u64) as usize
            }
        })
        .sum::<usize>();
    ((tail_bytes as u64).saturating_add(3)) / 4
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct BackgroundSnapshot {
    pub first_message_id: MessageId,
    pub initial: String,
    pub provider_id: String,
    pub model_id: String,
    pub variant: Option<String>,
    pub keep_message_id: MessageId,
    pub tail_limit: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct BackgroundContext {
    pub first_message_id: MessageId,
    pub initial: String,
    pub provider_id: String,
    pub model_id: String,
    pub variant: Option<String>,
    pub keep_message_id: MessageId,
    pub tail_tokens: u64,
}

#[derive(Debug)]
struct HeldBackground {
    snapshot: BackgroundSnapshot,
    result: Arc<Mutex<Option<Result<Vec<u8>, ()>>>>,
    ready: Arc<tokio::sync::Notify>,
    failures: u8,
    task: tokio::task::JoinHandle<()>,
}

/// Return whether a measured request is close enough to the configured limit
/// to justify work off the foreground path.  Keeping this small and pure is
/// intentional: callers can decide from a request snapshot without holding a
/// turn permit or touching the database twice.
pub(crate) fn background_due(estimated_tokens: u64, threshold_tokens: u64) -> bool {
    threshold_tokens != 0
        && estimated_tokens >= threshold_tokens.saturating_mul(BACKGROUND_RATIO) / 100
        && estimated_tokens < threshold_tokens
}

/// OpenResponses input_tokens is already inclusive of cached input. Keep the
/// authoritative total intact; cache details are metadata, not an addition.
pub(crate) fn measured_prompt_tokens(usage: &ReportedUsage) -> u64 {
    usage.input_tokens
}

fn workers() -> &'static Mutex<HashMap<SessionId, HeldBackground>> {
    BACKGROUND.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Start one non-persisting precompute operation from the real provider
/// history. The result is held until foreground compaction consumes it.
/// Repeated calls for the same anchor reuse the existing operation; after two
/// failed attempts upstream stops retrying until the next foreground compact.
pub(crate) fn start_background<F>(id: SessionId, snapshot: BackgroundSnapshot, operation: F) -> bool
where
    F: std::future::Future<Output = Result<Vec<u8>, ()>> + Send + 'static,
{
    let mut failures = 0;
    if let Ok(mut map) = workers().lock() {
        if let Some(existing) = map.get(&id) {
            let same_anchor = existing.snapshot.first_message_id == snapshot.first_message_id
                && existing.snapshot.keep_message_id == snapshot.keep_message_id;
            let compatible = same_anchor
                && existing.snapshot.initial == snapshot.initial
                && existing.snapshot.provider_id == snapshot.provider_id
                && existing.snapshot.model_id == snapshot.model_id
                && existing.snapshot.variant == snapshot.variant;
            if compatible {
                return false;
            }
            let observed_failure = existing
                .result
                .lock()
                .is_ok_and(|result| result.as_ref().is_some_and(Result::is_err));
            failures = existing.failures.saturating_add(u8::from(observed_failure));
            if failures >= 2 {
                return false;
            }
        }
        if let Some(previous) = map.remove(&id) {
            previous.task.abort();
        }
    }
    let result = Arc::new(Mutex::new(None));
    let result_slot = Arc::clone(&result);
    let ready = Arc::new(tokio::sync::Notify::new());
    let ready_slot = Arc::clone(&ready);
    let task = tokio::spawn(async move {
        let outcome = operation.await;
        if let Ok(mut slot) = result_slot.lock() {
            *slot = Some(outcome);
        }
        ready_slot.notify_waiters();
    });
    if let Ok(mut map) = workers().lock() {
        map.insert(
            id,
            HeldBackground {
                snapshot,
                result,
                ready,
                failures,
                task,
            },
        );
        true
    } else {
        task.abort();
        false
    }
}

/// Consume a held result only when it still describes the current session.
/// This is the stale-checkpoint guard: mismatch or an overgrown tail discards
/// the result without committing anything.
pub(crate) async fn take_if_usable(
    id: SessionId,
    current: &BackgroundContext,
    mut cancellation: Option<&mut tokio::sync::watch::Receiver<bool>>,
) -> Option<Vec<u8>> {
    let ready = {
        let map = workers().lock().ok()?;
        Arc::clone(&map.get(&id)?.ready)
    };
    loop {
        let notified = ready.notified();
        let pending = workers()
            .lock()
            .ok()?
            .get(&id)?
            .result
            .lock()
            .ok()?
            .is_none();
        if !pending {
            break;
        }
        if let Some(cancelled) = cancellation.as_deref_mut() {
            tokio::select! {
                biased;
                _ = cancelled.changed() => return None,
                _ = notified => {}
            }
        } else {
            notified.await;
        }
    }
    let mut map = workers().lock().ok()?;
    let held = map.get(&id)?;
    let same = held.snapshot.first_message_id == current.first_message_id
        && held.snapshot.initial == current.initial
        && held.snapshot.provider_id == current.provider_id
        && held.snapshot.model_id == current.model_id
        && held.snapshot.variant == current.variant
        && held.snapshot.keep_message_id == current.keep_message_id
        && current.tail_tokens <= held.snapshot.tail_limit;
    let result = held.result.lock().ok()?.take();
    if !same || result.as_ref().is_some_and(Result::is_err) {
        let stale = map.remove(&id).expect("held background entry exists");
        stale.task.abort();
        return None;
    }
    map.remove(&id);
    result.and_then(Result::ok)
}

pub(crate) fn cancel_background(id: SessionId) {
    if let Ok(mut map) = workers().lock() {
        if let Some(held) = map.remove(&id) {
            held.task.abort();
        }
    }
}

#[cfg(test)]
mod background_tests {
    use super::*;
    use tokio::time::{sleep, Duration};

    #[test]
    fn schedules_only_in_the_precompute_band() {
        assert!(!background_due(84, 100));
        assert!(background_due(85, 100));
        assert!(background_due(99, 100));
        assert!(!background_due(100, 100));
        assert!(!background_due(100, 0));
    }

    #[test]
    fn measured_prompt_uses_inclusive_provider_total() {
        let usage = ReportedUsage {
            input_tokens: 125_000,
            output_tokens: 32,
            cached_tokens: Some(120_000),
            reasoning_tokens: None,
        };
        assert_eq!(measured_prompt_tokens(&usage), 125_000);
    }

    #[test]
    fn request_context_matches_actual_provider_prefix_and_variant() {
        assert_eq!(
            request_context(None, ""),
            BackgroundRequestContext {
                initial: String::new(),
                variant: None,
            }
        );
        assert_eq!(
            request_context(Some("Agent: build\nA"), "low"),
            BackgroundRequestContext {
                initial: "Agent: build\nA".into(),
                variant: Some("low".into()),
            }
        );
    }

    fn snapshot_for(request: &BackgroundRequestContext, tail_limit: u64) -> BackgroundSnapshot {
        BackgroundSnapshot {
            first_message_id: MessageId::new(),
            initial: request.initial.clone(),
            provider_id: "p".into(),
            model_id: "m".into(),
            variant: request.variant.clone(),
            keep_message_id: MessageId::new(),
            tail_limit,
        }
    }

    fn snapshot() -> BackgroundSnapshot {
        snapshot_for(&request_context(Some("instructions"), ""), 10)
    }

    fn context_for(
        snapshot: &BackgroundSnapshot,
        request: &BackgroundRequestContext,
        tail_tokens: u64,
    ) -> BackgroundContext {
        BackgroundContext {
            first_message_id: snapshot.first_message_id,
            initial: request.initial.clone(),
            provider_id: snapshot.provider_id.clone(),
            model_id: snapshot.model_id.clone(),
            variant: request.variant.clone(),
            keep_message_id: snapshot.keep_message_id,
            tail_tokens,
        }
    }

    fn context(snapshot: &BackgroundSnapshot, tail_tokens: u64) -> BackgroundContext {
        let request = BackgroundRequestContext {
            initial: snapshot.initial.clone(),
            variant: snapshot.variant.clone(),
        };
        context_for(snapshot, &request, tail_tokens)
    }

    #[tokio::test]
    async fn holds_result_until_foreground_consumes_it() {
        let id = SessionId::new();
        let snap = snapshot();
        start_background(id, snap.clone(), async { Ok(b"summary".to_vec()) });
        sleep(Duration::from_millis(1)).await;
        assert_eq!(
            take_if_usable(id, &context(&snap, 1), None).await,
            Some(b"summary".to_vec())
        );
    }

    #[tokio::test]
    async fn repeated_pending_schedule_does_not_start_a_duplicate() {
        let id = SessionId::new();
        let snap = snapshot();
        let started = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let first = Arc::clone(&started);
        assert!(start_background(id, snap.clone(), async move {
            first.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            std::future::pending::<Result<Vec<u8>, ()>>().await
        }));
        assert!(!start_background(id, snap, async {
            Ok(b"duplicate".to_vec())
        }));
        assert_eq!(started.load(std::sync::atomic::Ordering::SeqCst), 0);
        cancel_background(id);
    }

    #[tokio::test]
    async fn completed_success_is_retained_and_not_restarted() {
        let id = SessionId::new();
        let snap = snapshot();
        assert!(start_background(id, snap.clone(), async {
            Ok(b"held".to_vec())
        }));
        sleep(Duration::from_millis(1)).await;
        assert!(!start_background(id, snap.clone(), async {
            Ok(b"second".to_vec())
        }));
        assert_eq!(
            take_if_usable(id, &context(&snap, 1), None).await,
            Some(b"held".to_vec())
        );
    }

    #[tokio::test]
    async fn rejects_tail_model_and_cancelled_replacements_without_commit() {
        let id = SessionId::new();
        let snap = snapshot();
        start_background(id, snap.clone(), async { Ok(b"stale".to_vec()) });
        sleep(Duration::from_millis(1)).await;
        assert_eq!(take_if_usable(id, &context(&snap, 11), None).await, None);

        let mut changed = snapshot();
        changed.model_id = "new-model".into();
        start_background(id, changed, async { Ok(b"new".to_vec()) });
        cancel_background(id);
        sleep(Duration::from_millis(1)).await;
        assert_eq!(take_if_usable(id, &context(&snap, 1), None).await, None);
    }

    #[tokio::test]
    async fn incompatible_same_anchor_replaces_and_old_result_is_not_consumed() {
        let id = SessionId::new();
        let old = snapshot();
        assert!(start_background(id, old.clone(), async {
            Ok(b"old".to_vec())
        }));
        sleep(Duration::from_millis(1)).await;
        let mut changed = old.clone();
        changed.provider_id = "other-provider".into();
        changed.model_id = "other-model".into();
        changed.variant = Some("other-variant".into());
        changed.initial = "other-instructions".into();
        assert!(start_background(id, changed.clone(), async {
            Ok(b"new".to_vec())
        }));
        sleep(Duration::from_millis(1)).await;
        assert_eq!(
            take_if_usable(id, &context(&changed, 1), None).await,
            Some(b"new".to_vec())
        );
    }

    #[tokio::test]
    async fn observed_failures_limit_changed_anchor_retries_but_cancel_does_not() {
        let id = SessionId::new();
        let first = snapshot();
        assert!(start_background(id, first.clone(), async { Err(()) }));
        sleep(Duration::from_millis(1)).await;
        let mut second = snapshot();
        second.keep_message_id = MessageId::new();
        assert!(start_background(id, second.clone(), async { Err(()) }));
        sleep(Duration::from_millis(1)).await;
        let mut third = snapshot();
        third.keep_message_id = MessageId::new();
        assert!(!start_background(id, third, async {
            Ok(b"blocked".to_vec())
        }));

        let cancel_id = SessionId::new();
        let cancel_snap = snapshot();
        assert!(start_background(cancel_id, cancel_snap.clone(), async {
            std::future::pending::<Result<Vec<u8>, ()>>().await
        }));
        cancel_background(cancel_id);
        let mut after_cancel = snapshot();
        after_cancel.keep_message_id = MessageId::new();
        assert!(start_background(cancel_id, after_cancel, async {
            Ok(b"retry".to_vec())
        }));
        cancel_background(cancel_id);
    }

    #[test]
    fn precomputed_tail_limit_uses_budget_cap_without_turn_multiplier() {
        assert_eq!(precomputed_tail_limit(100_000), 25_000);
        assert_eq!(precomputed_tail_limit(140_000), 30_000);
        assert_eq!(precomputed_tail_limit(200_000), 30_000);
        assert_eq!(precomputed_tail_limit(3), 0);
    }

    #[tokio::test]
    async fn completion_between_pending_check_and_wait_is_consumable() {
        let id = SessionId::new();
        let snap = snapshot();
        let (release, wait) = tokio::sync::oneshot::channel();
        start_background(id, snap.clone(), async move {
            let _ = wait.await;
            Ok(b"race-summary".to_vec())
        });
        let current_context = context(&snap, 1);
        let take = Box::pin(take_if_usable(id, &current_context, None));
        tokio::task::yield_now().await;
        let _ = release.send(());
        assert_eq!(take.await, Some(b"race-summary".to_vec()));
    }

    #[tokio::test]
    async fn production_request_context_initial_change_invalidates_same_anchor() {
        let id = SessionId::new();
        let first_request = request_context(Some("Agent: build\nA"), "low");
        let second_request = request_context(Some("Agent: build\nB"), "low");
        let snapshot = snapshot_for(&first_request, 10);
        assert!(start_background(id, snapshot.clone(), async {
            Ok(b"old-initial".to_vec())
        }));
        sleep(Duration::from_millis(1)).await;
        assert_eq!(
            take_if_usable(id, &context_for(&snapshot, &second_request, 10), None).await,
            None
        );
    }

    #[tokio::test]
    async fn production_request_context_variant_change_invalidates_same_anchor() {
        let id = SessionId::new();
        let first_request = request_context(Some("Agent: build\nA"), "low");
        let second_request = request_context(Some("Agent: build\nA"), "high");
        let snapshot = snapshot_for(&first_request, 10);
        assert!(start_background(id, snapshot.clone(), async {
            Ok(b"old-variant".to_vec())
        }));
        sleep(Duration::from_millis(1)).await;
        assert_eq!(
            take_if_usable(id, &context_for(&snapshot, &second_request, 10), None).await,
            None
        );
    }

    #[tokio::test]
    async fn unchanged_production_request_context_consumes_held_result_once() {
        let id = SessionId::new();
        let request = request_context(Some("Agent: build\nA"), "low");
        let snapshot = snapshot_for(&request, 10);
        assert!(start_background(id, snapshot.clone(), async {
            Ok(b"one-held-result".to_vec())
        }));
        sleep(Duration::from_millis(1)).await;
        assert!(!start_background(id, snapshot.clone(), async {
            Ok(b"duplicate".to_vec())
        }));
        assert_eq!(
            take_if_usable(id, &context_for(&snapshot, &request, 10), None).await,
            Some(b"one-held-result".to_vec())
        );
        assert_eq!(
            take_if_usable(id, &context_for(&snapshot, &request, 10), None).await,
            None
        );
    }

    #[tokio::test]
    async fn anchor_inclusive_tail_over_limit_rejects_held_result() {
        let id = SessionId::new();
        let request = request_context(None, "low");
        let snapshot = snapshot_for(&request, 10);
        let anchor = MessageRecord {
            id: MessageId::new(),
            session_id: id,
            role: MessageRole::Assistant,
            body: opencode_rk_contracts::PayloadRef::inline("a".repeat(41)).unwrap(),
            created_at: Timestamp::now(),
        };
        let suffix = MessageRecord {
            id: MessageId::new(),
            session_id: id,
            role: MessageRole::User,
            body: opencode_rk_contracts::PayloadRef::inline("b".repeat(3)).unwrap(),
            created_at: Timestamp::now(),
        };
        let messages = vec![anchor, suffix];
        let tail_tokens = retained_tail_tokens(&messages, 0);
        assert!(tail_tokens > snapshot.tail_limit);
        assert!(retained_tail_tokens(&messages, 1) <= snapshot.tail_limit);
        assert!(start_background(id, snapshot.clone(), async {
            Ok(b"over-limit".to_vec())
        }));
        sleep(Duration::from_millis(1)).await;
        assert_eq!(
            take_if_usable(id, &context_for(&snapshot, &request, tail_tokens), None).await,
            None
        );
    }

    #[tokio::test]
    async fn anchor_inclusive_tail_exact_limit_accepts_held_result() {
        let id = SessionId::new();
        let request = request_context(None, "low");
        let snapshot = snapshot_for(&request, 10);
        let anchor = MessageRecord {
            id: MessageId::new(),
            session_id: id,
            role: MessageRole::Assistant,
            body: opencode_rk_contracts::PayloadRef::inline("a".repeat(37)).unwrap(),
            created_at: Timestamp::now(),
        };
        let suffix = MessageRecord {
            id: MessageId::new(),
            session_id: id,
            role: MessageRole::User,
            body: opencode_rk_contracts::PayloadRef::inline("b".repeat(3)).unwrap(),
            created_at: Timestamp::now(),
        };
        let messages = vec![anchor, suffix];
        let tail_tokens = retained_tail_tokens(&messages, 0);
        assert_eq!(tail_tokens, snapshot.tail_limit);
        assert!(start_background(id, snapshot.clone(), async {
            Ok(b"exact-limit".to_vec())
        }));
        sleep(Duration::from_millis(1)).await;
        assert_eq!(
            take_if_usable(id, &context_for(&snapshot, &request, tail_tokens), None).await,
            Some(b"exact-limit".to_vec())
        );
    }
}

pub(crate) async fn settings(
    sessions: &SessionService,
    id: SessionId,
) -> Result<opencode_rk_sessions::SessionCompactionSettings, ApiFailure> {
    compaction_settings::load(sessions, id).await
}

/// Production turn-boundary entry point. It uses the just-persisted message
/// stream (not a synthetic snapshot) and starts only for fallback summaries
/// with a measured request in the upstream precompute band. The caller keeps
/// the returned foreground owner alive for the turn; a future integration may
/// supply the daemon's background owner for disconnect-independent lifetime.
pub(crate) async fn schedule_after_successful_turn(
    sessions: &SessionService,
    client: &OpenAiResponsesClient,
    owner: &Arc<RunOwner>,
    session_id: SessionId,
    provider_id: &str,
    model_id: &str,
    usage: Option<ReportedUsage>,
    request_context: &BackgroundRequestContext,
) {
    let Ok(cfg) = settings(sessions, session_id).await else {
        return;
    };
    if !cfg.enabled || cfg.strategy == opencode_rk_sessions::CompactionStrategy::Native {
        return;
    }
    let Ok(messages) = sessions
        .messages(session_id, MAX_RESPONSES_INPUT_MESSAGES)
        .await
    else {
        return;
    };
    let Some(first) = messages.first() else {
        return;
    };
    let Some(anchor) = messages
        .iter()
        .rev()
        .find(|message| message.role == MessageRole::Assistant)
    else {
        return;
    };
    let Ok((history, _)) = checkpoint_provider_history(
        sessions,
        session_id,
        &format!("{provider_id}/{model_id}"),
        MAX_RESPONSES_INPUT_MESSAGES,
        MAX_RESPONSES_INPUT_BYTES,
    )
    .await
    else {
        return;
    };
    let Some(usage) = usage.as_ref() else {
        return;
    };
    let measured = measured_prompt_tokens(usage);
    if !background_due(measured, cfg.threshold_tokens) || measured < 64_000 {
        return;
    }
    let snapshot = BackgroundSnapshot {
        first_message_id: first.id,
        initial: request_context.initial.clone(),
        provider_id: provider_id.to_owned(),
        model_id: model_id.to_owned(),
        variant: request_context.variant.clone(),
        keep_message_id: anchor.id,
        tail_limit: precomputed_tail_limit(cfg.threshold_tokens),
    };
    let sessions = sessions.clone();
    let client = client.clone();
    // Background precompute is tied to the durable session, not to the
    // foreground request owner (which may be cancelled when the turn ends).
    // Keep the parameter for the existing call boundary; cancellation is
    // represented by cancel_background/session lifecycle instead.
    let _ = owner;
    let provider_id = provider_id.to_owned();
    let model_id = model_id.to_owned();
    let initial = request_context.initial.clone();
    let variant = request_context.variant.clone().unwrap_or_default();
    start_background(session_id, snapshot, async move {
        fallback_compaction::prepare_background(
            &sessions,
            &client,
            session_id,
            None,
            &provider_id,
            &model_id,
            cfg.keep_recent_turns,
            &initial,
            &variant,
        )
        .await
        .map_err(|_| ())
    });
}

/// Perform compaction preflight while the turn permit is held, before a new
/// user row is appended.  A checkpoint is never deleted on provider failure.
pub(crate) async fn before_user_append(
    sessions: &SessionService,
    client: &OpenAiResponsesClient,
    owner: &Arc<RunOwner>,
    session_id: SessionId,
    provider_id: &str,
    model_id: &str,
    request_context: &BackgroundRequestContext,
    next_text: &str,
) -> Result<(), ApiFailure> {
    let cfg = settings(sessions, session_id).await?;
    if !cfg.enabled {
        return Ok(());
    }
    if owner.is_cancelled() {
        return Err(ApiFailure::bad_gateway("turn cancelled"));
    }

    // Capture the head before reading the input. If any writer appends after
    // this point, the existing transactional checkpoint guard rejects the
    // commit rather than treating unseen history as already compacted.
    let boundary = sessions
        .history_page(session_id, None, 1)
        .await
        .map_err(ApiFailure::internal)?
        .0
        .into_iter()
        .last();
    let Some(boundary) = boundary else {
        return Ok(());
    };

    let (history, previous_window) = checkpoint_provider_history(
        sessions,
        session_id,
        &format!("{provider_id}/{model_id}"),
        MAX_RESPONSES_INPUT_MESSAGES,
        MAX_RESPONSES_INPUT_BYTES,
    )
    .await?;
    if history.is_empty() {
        return Ok(());
    }
    let mut canonical = previous_window
        .as_ref()
        .map_or_else(Vec::new, |window| window.output.clone());
    canonical.extend(
        history
            .iter()
            .map(serde_json::to_value)
            .collect::<Result<Vec<_>, _>>()
            .map_err(ApiFailure::internal)?,
    );
    let request = opencode_rk_providers::responses::responses_request_payload(
        model_id,
        "",
        &canonical,
        &[],
        false,
    )
    .map_err(provider_failure)?;
    // Include the opaque window and request envelope in this explicitly
    // estimated count; none of it is decoded or presented as measured usage.
    let bytes = serde_json::to_vec(&request)
        .map_err(ApiFailure::internal)?
        .len();
    let estimated_tokens = ((bytes as u64).saturating_add(3) / 4)
        .saturating_add(((next_text.len() as u64).saturating_add(3)) / 4);
    // Reserve the prospective user row, current instruction block, and an
    // optional internal agent prompt. These are rebuilt after compaction.
    if estimated_tokens < cfg.threshold_tokens
        && canonical.len().saturating_add(3) <= MAX_RESPONSES_INPUT_MESSAGES
        && bytes.saturating_add(next_text.len()) < MAX_RESPONSES_INPUT_BYTES
    {
        return Ok(());
    }

    if cfg.strategy != opencode_rk_sessions::CompactionStrategy::Native {
        let current_messages = sessions
            .messages(session_id, MAX_RESPONSES_INPUT_MESSAGES)
            .await
            .map_err(ApiFailure::internal)?;
        let Some(first) = current_messages.first() else {
            return Ok(());
        };
        let Some(keep) = current_messages
            .iter()
            .rev()
            .find(|message| message.role == MessageRole::Assistant)
        else {
            return fallback_compaction::compact(
                sessions,
                client,
                owner,
                provider_id,
                model_id,
                cfg.keep_recent_turns,
            )
            .await;
        };
        // `bytes` is the complete provider request and therefore includes the
        // old prefix.  The held summary is anchored at `keep.id`; only the
        // persisted records after that anchor are the retained tail checked
        // by take_if_usable.  Counting the whole request made an immediately
        // consumable result look stale and caused a second summary request.
        let anchor_index = current_messages
            .iter()
            .rposition(|message| message.id == keep.id)
            .unwrap_or(current_messages.len().saturating_sub(1));
        let tail_tokens = retained_tail_tokens(&current_messages, anchor_index);
        let context = BackgroundContext {
            first_message_id: first.id,
            initial: request_context.initial.clone(),
            provider_id: provider_id.to_owned(),
            model_id: model_id.to_owned(),
            variant: request_context.variant.clone(),
            keep_message_id: keep.id,
            tail_tokens,
        };
        let mut cancelled = owner.cancellation_receiver();
        if let Some(window_json) = take_if_usable(session_id, &context, Some(&mut cancelled)).await
        {
            let window_json = String::from_utf8(window_json)
                .map_err(|_| ApiFailure::internal("invalid held compaction checkpoint"))?;
            return commit_compaction_checkpoint(
                sessions.clone(),
                owner.clone(),
                CompactionCheckpoint {
                    boundary_message_id: boundary.id,
                    provider_id: provider_id.to_owned(),
                    model_id: model_id.to_owned(),
                    window_json,
                },
            )
            .await;
        }
        return fallback_compaction::compact(
            sessions,
            client,
            owner,
            provider_id,
            model_id,
            cfg.keep_recent_turns,
        )
        .await;
    }

    let mut cancelled = owner.cancellation_receiver();
    let window = tokio::select! {
        biased;
        _ = cancelled.changed() => return Err(ApiFailure::bad_gateway("compaction cancelled")),
        result = async {
            match previous_window.as_ref() {
                Some(window) => client.compact_with_compaction_window(model_id, window, &history).await,
                None => client.compact(model_id, &history).await,
            }
        } => result.map_err(provider_failure)?,
    };
    if owner.is_cancelled() {
        return Err(ApiFailure::bad_gateway("compaction cancelled"));
    }
    let window_json = serde_json::to_string(&json!({
        "id": window.id, "object":"response.compaction", "created_at":window.created_at,
        "usage":window.usage, "output":window.output
    }))
    .map_err(ApiFailure::internal)?;
    super::commit_compaction_checkpoint(
        sessions.clone(),
        owner.clone(),
        CompactionCheckpoint {
            boundary_message_id: boundary.id,
            provider_id: provider_id.to_owned(),
            model_id: model_id.to_owned(),
            window_json,
        },
    )
    .await
}
