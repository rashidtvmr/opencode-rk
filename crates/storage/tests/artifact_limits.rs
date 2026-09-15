use opencode_rk_contracts::{
    ArtifactKind, MessageId, SessionId, Timestamp, MAX_ARTIFACTS_PER_SESSION,
    MAX_ARTIFACT_VERSIONS,
};
use opencode_rk_storage::{Storage, StorageError};
use tempfile::tempdir;

#[test]
fn web_017_artifact_and_version_counts_are_hard_bounded() {
    let dir = tempdir().unwrap();
    let storage = Storage::open_in_memory(dir.path().join("blobs")).unwrap();
    let session_id = SessionId::new();
    let first = storage
        .create_artifact(
            session_id,
            MessageId::new(),
            ArtifactKind::Writing,
            "Bounded draft",
            None,
            "v1",
            Timestamp::now(),
        )
        .unwrap();

    for version in 2..=MAX_ARTIFACT_VERSIONS {
        let document = storage
            .append_artifact_version(
                session_id,
                first.summary.id,
                &format!("v{version}"),
                Timestamp::now(),
            )
            .unwrap();
        assert_eq!(document.summary.current_version as usize, version);
    }
    assert!(matches!(
        storage.append_artifact_version(
            session_id,
            first.summary.id,
            "one too many",
            Timestamp::now(),
        ),
        Err(StorageError::ArtifactLimitExceeded)
    ));

    for index in 1..MAX_ARTIFACTS_PER_SESSION {
        storage
            .create_artifact(
                session_id,
                MessageId::new(),
                ArtifactKind::Code,
                &format!("Code {index}"),
                Some("text"),
                "x",
                Timestamp::now(),
            )
            .unwrap();
    }
    assert_eq!(
        storage
            .list_artifacts(session_id, MAX_ARTIFACTS_PER_SESSION)
            .unwrap()
            .len(),
        MAX_ARTIFACTS_PER_SESSION
    );
    assert!(matches!(
        storage.create_artifact(
            session_id,
            MessageId::new(),
            ArtifactKind::Writing,
            "Overflow",
            None,
            "x",
            Timestamp::now(),
        ),
        Err(StorageError::ArtifactLimitExceeded)
    ));
}
