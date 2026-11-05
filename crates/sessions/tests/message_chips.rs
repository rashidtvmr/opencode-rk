//! MSG-005: message-level user-card chips (`Session.Message.User.files`/
//! `.skills`, `packages/schema/src/session-message.ts:76-84`) round-trip with
//! the user prompt they belong to, and a chip-less prompt stores none.

use std::sync::Arc;

use opencode_rk_contracts::{MessageFile, MessageRole, MessageSkill};
use opencode_rk_sessions::SessionService;
use opencode_rk_storage::Storage;

fn service() -> (SessionService, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let storage = Arc::new(Storage::open_in_memory(dir.path().join("blobs")).unwrap());
    (SessionService::new(storage), dir)
}

#[tokio::test]
async fn user_prompt_chips_round_trip_with_the_message() {
    let (sessions, _dir) = service();
    let session = sessions.create("chips").await.unwrap();
    let files = vec![
        MessageFile {
            name: "main.rs".into(),
            mime: "text/x-rust".into(),
            hash: Some("sha-main".into()),
        },
        MessageFile {
            name: "src".into(),
            mime: "application/x-directory".into(),
            hash: None,
        },
    ];
    let skills = vec![MessageSkill {
        id: "skill-commit".into(),
        name: "commit".into(),
    }];
    let message = sessions
        .append_user_prompt(session.id, "please commit", files.clone(), skills.clone())
        .await
        .unwrap();
    assert_eq!(message.role, MessageRole::User);
    assert_eq!(message.files, files);
    assert_eq!(message.skills, skills);
    // The chips are durable message facts, not transient return values.
    let stored = sessions.messages(session.id, 10).await.unwrap();
    let stored = stored
        .iter()
        .find(|stored| stored.id == message.id)
        .expect("stored user message");
    assert_eq!(stored.files, files);
    assert_eq!(stored.skills, skills);
}

#[tokio::test]
async fn chip_less_prompts_store_no_chips() {
    let (sessions, _dir) = service();
    let session = sessions.create("no chips").await.unwrap();
    let message = sessions
        .append_text(session.id, MessageRole::User, "plain prompt")
        .await
        .unwrap();
    assert!(message.files.is_empty());
    assert!(message.skills.is_empty());
    let stored = sessions.messages(session.id, 10).await.unwrap();
    let stored = stored
        .iter()
        .find(|stored| stored.id == message.id)
        .expect("stored user message");
    assert!(stored.files.is_empty());
    assert!(stored.skills.is_empty());
}
