use opencode_rk_contracts::SessionId;
use opencode_rk_storage::{fork_v2::ForkV2, NewSession, SchemaV2, V2Writer};
use rusqlite::params;

#[test]
fn web_008_t01_fork_inherits_stable_parent_session_configuration() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("workspace.db");
    let mut conn = SchemaV2::initialize_workspace(&path, [1_u8; 16], [2_u8; 16], 10).unwrap();
    let parent = SessionId::new();
    V2Writer::create_session(
        &mut conn,
        &NewSession {
            id: parent,
            title: "Parent".to_owned(),
            created_at_us: 100,
            updated_at_us: 100,
        },
    )
    .unwrap();
    conn.execute(
        "UPDATE sessions SET agent_name=?1, provider_id=?2, model_id=?3, effort_json=?4 WHERE id=?5",
        params![
            "coding-agent",
            "openai",
            "gpt-5.6",
            r#"{"reasoning_effort":"high"}"#,
            parent.as_uuid().as_bytes().as_slice(),
        ],
    )
    .unwrap();

    let child = SessionId::new();
    let child_pk = ForkV2::fork_session(
        &mut conn,
        parent.as_uuid().as_bytes().as_slice(),
        0,
        child,
        "Branch: Parent",
        200,
    )
    .unwrap();
    let config: (String, Option<String>, Option<String>, Option<String>) = conn
        .query_row(
            "SELECT agent_name,provider_id,model_id,effort_json FROM sessions WHERE pk=?1",
            params![child_pk],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .unwrap();
    assert_eq!(config.0, "coding-agent");
    assert_eq!(config.1.as_deref(), Some("openai"));
    assert_eq!(config.2.as_deref(), Some("gpt-5.6"));
    assert_eq!(config.3.as_deref(), Some(r#"{"reasoning_effort":"high"}"#));
}
