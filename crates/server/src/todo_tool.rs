//! Session-scoped todo mutation through the ordinary model tool permission gate.
use crate::{event_bus::{EventBus, ServerEvent}, run_owner::{RunOwner, PendingPermissionGuard,
    PermissionRequest, PermissionSource, PermissionReply}, AppState, NativeToolOutcome};
use opencode_rk_contracts::MessageId;
use opencode_rk_providers::responses::ResponsesTool;
use opencode_rk_security::{Decision, OperationIntent, PermissionBroker};
use opencode_rk_sessions::{validate_todos, Todo};
use serde::Deserialize;
use serde_json::json;
use std::sync::Arc;

pub(crate) fn schema() -> ResponsesTool {
    ResponsesTool::function("todowrite", "Create and maintain the current session's ordered task list.", json!({
        "type":"object","additionalProperties":false,"required":["todos"],"properties":{
            "todos":{"type":"array","maxItems":100,"items":{"type":"object","additionalProperties":false,
                "required":["id","content","status","priority"],"properties":{
                    "id":{"type":"string","minLength":1,"maxLength":128},
                    "content":{"type":"string","minLength":1,"maxLength":4096},
                    "status":{"type":"string","enum":["pending","in_progress","completed","cancelled"]},
                    "priority":{"type":"string","enum":["high","medium","low"]}
                }}}
        }
    }))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Input { todos: Vec<Todo> }

pub(crate) async fn execute(
    app: &AppState, events: &EventBus, owner: &Arc<RunOwner>, broker: &PermissionBroker,
    message_id: MessageId, call_id: &str, arguments: &str,
) -> NativeToolOutcome {
    let fail = |message: &str, stop| NativeToolOutcome {
        output: format!("error: {message}"), success: false, stop_after_outputs: stop,
    };
    if arguments.len() > 64 * 1024 { return fail("todo arguments exceed size limit", false); }
    let input = match serde_json::from_str::<Input>(arguments) {
        Ok(input) if validate_todos(&input.todos).is_ok() => input,
        _ => return fail("invalid todo collection", false),
    };
    // Bind the permission to these exact arguments and this session/run/call.
    let intent = OperationIntent::Tool { name: "todowrite".into(), description: arguments.to_owned() };
    if crate::permission_rules::denied("todowrite", &owner.session_id.to_string(), &intent, broker) {
        return fail("todowrite denied by permission rule", false);
    }
    match broker.authorize(&intent) {
        Decision::Allow => {}
        Decision::Deny { .. } => return fail("todowrite denied", false),
        Decision::RequireHuman { .. } => {
            let resource = match serde_json::to_string(&intent) {
                Ok(resource) => resource,
                Err(_) => return fail("invalid todo permission intent", false),
            };
            let request = PermissionRequest {
                id: format!("per_{}", opencode_rk_contracts::ApprovalId::new()),
                session_id: owner.session_id.to_string(), action: "todowrite".into(), resources: vec![resource],
                source: PermissionSource { kind: "tool", message_id: message_id.to_string(), call_id: call_id.to_owned() },
            };
            let request_id = request.id.clone();
            let receiver = match owner.install_pending(request, intent.clone(), broker) {
                Ok(receiver) => receiver,
                Err(_) => return fail("todo approval unavailable", true),
            };
            let _pending = PendingPermissionGuard::new(owner.clone(), request_id);
            let mut cancelled = owner.cancellation_receiver();
            let approval = tokio::select! {
                biased;
                _ = cancelled.changed() => return fail("todowrite cancelled", true),
                approval = receiver => match approval { Ok(approval) => approval, Err(_) => return fail("todo approval unavailable", true) },
            };
            if !matches!(approval.reply, PermissionReply::Once | PermissionReply::Always) {
                return fail("todowrite rejected or cancelled", true);
            }
            let Some(proof) = approval.capability else { return fail("todo approval proof missing", true); };
            if !matches!(broker.authorize_with_human_approval(&intent, proof, &owner.session_id.to_string(), owner.run_id, call_id), Decision::Allow) {
                return fail("todo approval invalid", true);
            }
        }
    }
    let sessions = app.sessions.clone();
    let worker_owner = Arc::clone(owner);
    let todos = input.todos.clone();
    // Check cancellation when the worker actually starts, not before queueing.
    // Keep the same effect gate through the complete synchronous transaction,
    // so another effect cannot enter while this replacement is committing.
    let replacement = tokio::task::spawn_blocking(move || {
        let Some(_effect) = worker_owner.begin_effect() else { return Ok(false); };
        sessions.replace_todos_blocking(worker_owner.session_id, &todos).map(|()| true)
    }).await;
    match replacement {
        Ok(Ok(true)) => {}
        Ok(Ok(false)) => return fail("todowrite cancelled", true),
        _ => return fail("unable to persist todos", false),
    }
    let _ = events.publish(ServerEvent::TodoUpdated { session: owner.session_id, todos: input.todos.clone() });
    NativeToolOutcome {
        output: serde_json::to_string(&input.todos).expect("validated todo serialization"),
        success: true, stop_after_outputs: false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{future::Future, task::Poll};

    struct Release(Option<std::sync::mpsc::Sender<()>>);
    impl Drop for Release {
        fn drop(&mut self) { if let Some(sender) = self.0.take() { let _ = sender.send(()); } }
    }
    struct RulesEnvironment(Option<std::ffi::OsString>);
    impl Drop for RulesEnvironment {
        fn drop(&mut self) {
            if let Some(path) = &self.0 { std::env::set_var("OPENCODE_RK_PERMISSION_RULES", path); }
            else { std::env::remove_var("OPENCODE_RK_PERMISSION_RULES"); }
        }
    }

    #[test]
    fn cancellation_before_queued_storage_starts_does_not_mutate_todos() {
        let runtime = tokio::runtime::Builder::new_current_thread().enable_all()
            .max_blocking_threads(1).build().unwrap();
        runtime.block_on(async {
            let directory = tempfile::tempdir().unwrap();
            let _rules = RulesEnvironment(std::env::var_os("OPENCODE_RK_PERMISSION_RULES"));
            std::env::set_var("OPENCODE_RK_PERMISSION_RULES", directory.path().join("rules.json"));
            let sessions = opencode_rk_sessions::SessionService::new(Arc::new(
                opencode_rk_storage::Storage::open_in_memory(directory.path().join("blobs")).unwrap()));
            let session = sessions.create("Cancelled todo update").await.unwrap();
            let app = AppState { sessions, catalog: Arc::new(opencode_rk_catalog::Catalog::default()) };
            let events = EventBus::new(2);
            let mut subscription = events.subscribe(None);
            let permit = crate::run_owner::TurnExecutionPermit::acquire(session.id).unwrap();
            let owner = permit.owner();
            let broker = PermissionBroker::new(opencode_rk_security::SecurityPolicy::lean_default(directory.path()));
            let (started_tx, started_rx) = tokio::sync::oneshot::channel();
            let (release_tx, release_rx) = std::sync::mpsc::channel();
            let release = Release(Some(release_tx));
            let blocker = tokio::task::spawn_blocking(move || {
                let _ = started_tx.send(());
                let _ = release_rx.recv();
            });
            started_rx.await.unwrap();
            let arguments = json!({"todos":[{"id":"cancelled","content":"Must not persist",
                "status":"pending","priority":"medium"}]}).to_string();
            let work = execute(&app, &events, &owner, &broker, MessageId::new(), "cancelled-todo", &arguments);
            tokio::pin!(work);
            // Poll through authorization and queue the storage operation while
            // the sole blocking worker is held. No sleeps or scheduler guesses.
            std::future::poll_fn(|cx| {
                assert!(work.as_mut().poll(cx).is_pending());
                Poll::Ready(())
            }).await;
            owner.cancel();
            drop(release);
            blocker.await.unwrap();
            let outcome = work.await;
            assert!(!outcome.success, "a cancelled queued update must not commit");
            assert!(app.sessions.todos(session.id).await.unwrap().is_empty());
            assert!(subscription.try_recv().is_err(), "a cancelled update must not publish");
        });
    }
}
