use std::sync::Arc;

use axum::{
    body::{to_bytes, Body},
    http::{header, Request, StatusCode},
};
use opencode_rk_catalog::Catalog;
use opencode_rk_server::{router, AppState};
use opencode_rk_sessions::SessionService;
use opencode_rk_storage::Storage;
use tempfile::tempdir;
use tower::ServiceExt;

fn app() -> axum::Router {
    let dir = tempdir().expect("temporary server fixture");
    let storage = Storage::open_in_memory(dir.path().join("blobs")).expect("storage fixture");
    router(AppState {
        sessions: SessionService::new(Arc::new(storage)),
        catalog: Arc::new(Catalog::default()),
    })
}

#[tokio::test]
async fn web_006_t03_embedded_web_entrypoint_shares_the_native_api_origin() {
    let app = app();
    let index = app
        .clone()
        .oneshot(Request::builder().uri("/").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(index.status(), StatusCode::OK);
    assert_eq!(
        index.headers()[header::CONTENT_TYPE],
        "text/html; charset=utf-8"
    );
    let index_body = to_bytes(index.into_body(), 128 * 1024).await.unwrap();
    let index_text = std::str::from_utf8(&index_body).unwrap();
    assert!(index_text.contains("<title>OpenCode RK</title>"));
    assert!(index_text.contains("/assets/"));

    let spa_route = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/chat/local-session")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(spa_route.status(), StatusCode::OK);
    assert_eq!(
        spa_route.headers()[header::CONTENT_TYPE],
        "text/html; charset=utf-8"
    );

    let missing_api = app
        .oneshot(
            Request::builder()
                .uri("/api/not-a-real-endpoint")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(missing_api.status(), StatusCode::NOT_FOUND);
}
