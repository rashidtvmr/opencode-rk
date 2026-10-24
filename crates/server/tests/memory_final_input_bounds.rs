//! Final context admission includes serialized system instructions, not just history.
use axum::{body::{to_bytes, Body}, http::{Request, StatusCode}};
use opencode_rk_catalog::Catalog;
use opencode_rk_contracts::MessageRole;
use opencode_rk_server::{router, AppState};
use opencode_rk_sessions::SessionService;
use opencode_rk_storage::Storage;
use serde_json::{json, Value};
use std::sync::Arc;
use tower::ServiceExt;

struct Environment(Vec<(&'static str, Option<std::ffi::OsString>)>);
impl Drop for Environment { fn drop(&mut self) {
    for (key, old) in &self.0 { if let Some(old) = old { std::env::set_var(key, old); } else { std::env::remove_var(key); } }
} }

#[tokio::test]
async fn full_selected_context_is_rejected_explicitly_without_losing_history() {
    let _environment = Environment(["OPENAI_API_KEY", "OPENAI_BASE_URL"].into_iter().map(|key| (key, std::env::var_os(key))).collect());
    std::env::set_var("OPENAI_API_KEY", "context-bound-fixture-key");
    std::env::set_var("OPENAI_BASE_URL", "http://127.0.0.1:9");
    for (count, bytes) in [(99, 16), (8, 60_000)] {
        let root = tempfile::tempdir().unwrap();
        let instruction = "m".repeat(60_000);
        std::fs::write(root.path().join("AGENTS.md"), &instruction).unwrap();
        let sessions = SessionService::new(Arc::new(Storage::open_in_memory(root.path().join("blobs")).unwrap()));
        let session = sessions.create_in_directory("Final context bound", Some(root.path().to_owned())).await.unwrap();
        for index in 0..count {
            sessions.append_text(session.id, if index % 2 == 0 { MessageRole::User } else { MessageRole::Assistant }, "h".repeat(bytes)).await.unwrap();
        }
        let app = router(AppState { sessions: sessions.clone(), catalog: Arc::new(Catalog::default()) });
        let response = app.oneshot(Request::builder().method("POST").uri(format!("/api/sessions/{}/turns", session.id))
            .header("content-type", "application/json").body(Body::from(json!({"model":"openai/fixture","text":"Keep the latest request"}).to_string())).unwrap()).await.unwrap();
        let status = response.status();
        let body: Value = serde_json::from_slice(&to_bytes(response.into_body(), 64 * 1024).await.unwrap()).unwrap();
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "selected context must be admitted after system prompts: {body}");
        assert_eq!(body["code"], "context_budget_exceeded");
        assert_eq!(sessions.messages(session.id, 200).await.unwrap().len(), count + 1, "budget admission must not delete transcript records");
        assert_eq!(std::fs::read_to_string(root.path().join("AGENTS.md")).unwrap(), instruction);
    }
}
