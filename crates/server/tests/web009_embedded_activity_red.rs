//! WEB-009 RED: the embedded production bundle exposes structured activity UI.
//!
//! This test deliberately crosses the native router boundary. It does not read
//! web source files or inspect a generated file directly: the index document and
//! module bundle must both be returned by the same production router.
#![forbid(unsafe_code)]

use std::sync::Arc;

use axum::{
    body::{to_bytes, Body},
    http::{header, Request, StatusCode},
};
use opencode_rk_catalog::Catalog;
use opencode_rk_server::{
    daemon_auth::DaemonAuth,
    router_with_auth,
    AppState,
};
use opencode_rk_sessions::SessionService;
use opencode_rk_storage::Storage;
use tempfile::tempdir;
use tower::ServiceExt;

const INDEX_BODY_LIMIT: usize = 128 * 1024;
const SCRIPT_BODY_LIMIT: usize = 2 * 1024 * 1024;
const AUTH_TOKEN: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

fn app() -> axum::Router {
    let dir = tempdir().expect("temporary server fixture");
    let storage = Storage::open_in_memory(dir.path().join("blobs")).expect("storage fixture");
    let auth = DaemonAuth::from_published(AUTH_TOKEN).expect("fixture auth token");
    router_with_auth(
        AppState {
            sessions: SessionService::new(Arc::new(storage)),
            catalog: Arc::new(Catalog::default()),
        },
        Some(auth),
    )
}

fn attribute<'a>(tag: &'a str, wanted: &str) -> Option<&'a str> {
    let bytes = tag.as_bytes();
    let mut cursor = 0;
    while cursor < bytes.len() {
        while cursor < bytes.len() && bytes[cursor].is_ascii_whitespace() {
            cursor += 1;
        }
        let key_start = cursor;
        while cursor < bytes.len()
            && (bytes[cursor].is_ascii_alphanumeric() || bytes[cursor] == b'-')
        {
            cursor += 1;
        }
        if key_start == cursor {
            cursor += 1;
            continue;
        }
        let key_end = cursor;
        while cursor < bytes.len() && bytes[cursor].is_ascii_whitespace() {
            cursor += 1;
        }
        if cursor == bytes.len() || bytes[cursor] != b'=' {
            continue;
        }
        cursor += 1;
        while cursor < bytes.len() && bytes[cursor].is_ascii_whitespace() {
            cursor += 1;
        }
        if cursor == bytes.len() {
            break;
        }
        let quote = matches!(bytes[cursor], b'"' | b'\'').then_some(bytes[cursor]);
        if let Some(quote) = quote {
            cursor += 1;
            let value_start = cursor;
            while cursor < bytes.len() && bytes[cursor] != quote {
                cursor += 1;
            }
            let value_end = cursor;
            if cursor < bytes.len() {
                cursor += 1;
            }
            if tag[key_start..key_end].eq_ignore_ascii_case(wanted) {
                return Some(&tag[value_start..value_end]);
            }
        } else {
            let value_start = cursor;
            while cursor < bytes.len() && !bytes[cursor].is_ascii_whitespace() {
                cursor += 1;
            }
            if tag[key_start..key_end].eq_ignore_ascii_case(wanted) {
                return Some(&tag[value_start..cursor]);
            }
        }
    }
    None
}

fn opening_script_tags(html: &str) -> impl Iterator<Item = &str> {
    html.split("<script")
        .skip(1)
        .filter_map(|tail| tail.split_once('>').map(|(tag, _)| tag))
}

fn bounded_asset_path(path: &str) {
    assert!(!path.is_empty() && path.len() <= 512, "unbounded script path");
    assert!(path.starts_with("/assets/"), "script path escapes asset root: {path:?}");
    assert!(!path.starts_with("//"), "protocol-relative script URL: {path:?}");
    assert!(!path.contains("://"), "external script URL: {path:?}");
    assert!(
        !path.chars().any(|character| character.is_control() || character.is_whitespace()),
        "script path contains unsafe characters: {path:?}"
    );
    assert!(
        !path.split('/').any(|segment| matches!(segment, "." | "..")),
        "script path escapes asset root: {path:?}"
    );
}

#[tokio::test]
async fn web_009_embedded_production_bundle_contains_activity_labels() {
    let app = app();
    let index = app
        .clone()
        .oneshot(Request::builder().uri("/").body(Body::empty()).unwrap())
        .await
        .expect("index response");
    assert_eq!(index.status(), StatusCode::OK);
    assert_eq!(
        index.headers()[header::CONTENT_TYPE],
        "text/html; charset=utf-8"
    );
    let index_body = to_bytes(index.into_body(), INDEX_BODY_LIMIT)
        .await
        .expect("bounded index body");
    assert!(!index_body.is_empty());
    let index_text = std::str::from_utf8(&index_body).expect("UTF-8 index document");

    let script_sources: Vec<&str> = opening_script_tags(index_text)
        .filter_map(|tag| attribute(tag, "src"))
        .collect();
    assert!(!script_sources.is_empty(), "index must declare a script asset");
    for source in &script_sources {
        bounded_asset_path(source);
    }

    let module_sources: Vec<&str> = opening_script_tags(index_text)
        .filter(|tag| attribute(tag, "type") == Some("module"))
        .filter_map(|tag| attribute(tag, "src"))
        .collect();
    assert_eq!(module_sources.len(), 1, "exactly one module script is required");
    let module_path = module_sources[0];
    bounded_asset_path(module_path);

    let script = app
        .oneshot(
            Request::builder()
                .uri(module_path)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .expect("module response");
    assert_eq!(script.status(), StatusCode::OK);
    let content_type = script
        .headers()
        .get(header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .expect("module content type");
    assert!(
        content_type.starts_with("text/javascript")
            || content_type.starts_with("application/javascript"),
        "unexpected JavaScript content type: {content_type}"
    );
    let script_body = to_bytes(script.into_body(), SCRIPT_BODY_LIMIT)
        .await
        .expect("bounded module body");
    assert!(!script_body.is_empty());
    let script_text = std::str::from_utf8(&script_body).expect("UTF-8 JavaScript bundle");

    for label in ["Tool activity", "Reasoning summary", "References"] {
        assert!(
            script_text.contains(label),
            "embedded production bundle is missing exact user-visible label {label:?}"
        );
    }
    assert!(
        script_text.contains("https:"),
        "embedded bundle must preserve HTTPS-only reference semantics"
    );
    assert!(
        script_text.contains("noreferrer"),
        "embedded bundle must preserve noreferrer link semantics"
    );
}
