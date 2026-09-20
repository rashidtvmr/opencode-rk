//! Embedded production web client served by the native singleton daemon.
//!
//! Fail-closed bundle contract: the release bundle is embedded at compile
//! time from `$CARGO_MANIFEST_DIR/web_dist` (emitted by
//! `web/vite.config.ts`). When the bundle entry (`index.html`) is absent the
//! handler answers `503 Service Unavailable` with an explicit rebuild message
//! instead of a bare 404, so a missing/stale bundle can never masquerade as a
//! missing route. `/api/*` keeps its 404 contract regardless of bundle state.
//!
//! Capabilities honesty seam: [`bundle_available`] probes the real embedded
//! write-path (entry document present). The `/api/capabilities` surface reads
//! this so a missing bundle is reported unavailable with a reason instead of
//! silently serving 404s.

use axum::{
    body::Body,
    http::{StatusCode, Uri, header},
    response::{IntoResponse, Response},
};
use include_dir::{Dir, include_dir};

static WEB_DIST: Dir<'_> = include_dir!("$CARGO_MANIFEST_DIR/web_dist");

/// Bundle entry document; direct target for `/` and the SPA fallback.
pub const BUNDLE_ENTRY: &str = "index.html";

/// Fail-closed message served (and reported to capabilities) when the
/// embedded bundle carries no entry document.
pub const BUNDLE_MISSING_MESSAGE: &str = "web bundle unavailable: web_dist/index.html is not embedded; rebuild the web bundle and restart the daemon";

/// Real write-path probe: true exactly when the embedded bundle carries the
/// entry document the handler serves.
#[must_use]
pub fn bundle_available() -> bool {
    WEB_DIST.get_file(BUNDLE_ENTRY).is_some()
}

pub async fn serve(uri: Uri) -> Response {
    let path = uri.path().trim_start_matches('/');
    if path == "health" || path == "api" || path.starts_with("api/") {
        return StatusCode::NOT_FOUND.into_response();
    }
    serve_path(path, |name| {
        WEB_DIST.get_file(name).map(|file| file.contents())
    })
}

fn serve_path(path: &str, lookup: impl Fn(&str) -> Option<&'static [u8]>) -> Response {
    if path.contains("..") || path.contains('\0') {
        return StatusCode::BAD_REQUEST.into_response();
    }
    if lookup(BUNDLE_ENTRY).is_none() {
        return (StatusCode::SERVICE_UNAVAILABLE, BUNDLE_MISSING_MESSAGE).into_response();
    }

    let requested = if path.is_empty() { BUNDLE_ENTRY } else { path };
    if let Some(bytes) = lookup(requested) {
        return asset_response(requested, bytes);
    }

    if !requested
        .rsplit('/')
        .next()
        .is_some_and(|name| name.contains('.'))
    {
        if let Some(index) = lookup(BUNDLE_ENTRY) {
            return asset_response(BUNDLE_ENTRY, index);
        }
    }

    StatusCode::NOT_FOUND.into_response()
}

fn asset_response(path: &str, bytes: &'static [u8]) -> Response {
    let content_type = match path.rsplit('.').next() {
        Some("html") => "text/html; charset=utf-8",
        Some("js") => "text/javascript; charset=utf-8",
        Some("css") => "text/css; charset=utf-8",
        Some("woff2") => "font/woff2",
        Some("json") => "application/json; charset=utf-8",
        Some("svg") => "image/svg+xml",
        Some("png") => "image/png",
        Some("jpg" | "jpeg") => "image/jpeg",
        Some("webp") => "image/webp",
        _ => "application/octet-stream",
    };
    let cache_control = if path == "index.html" {
        "no-cache"
    } else {
        "public, max-age=31536000, immutable"
    };
    (
        StatusCode::OK,
        [
            (header::CONTENT_TYPE, content_type),
            (header::CACHE_CONTROL, cache_control),
            (header::X_CONTENT_TYPE_OPTIONS, "nosniff"),
        ],
        Body::from(bytes),
    )
        .into_response()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn embedded(name: &str) -> Option<&'static [u8]> {
        WEB_DIST.get_file(name).map(|file| file.contents())
    }

    fn fixture(name: &str) -> Option<&'static [u8]> {
        match name {
            "index.html" => Some(b"<title>OpenCode RK</title>" as &[u8]),
            "assets/app.js" => Some(b"console.log(1)" as &[u8]),
            _ => None,
        }
    }

    #[test]
    fn embedded_bundle_carries_entry() {
        assert!(bundle_available(), "release bundle must embed index.html");
        let index = embedded(BUNDLE_ENTRY).expect("entry document present");
        assert!(!index.is_empty());
        let text = std::str::from_utf8(index).expect("entry is utf-8 html");
        assert!(text.contains("<title>OpenCode RK</title>"));
    }

    #[tokio::test]
    async fn entry_and_api_contract_on_real_bundle() {
        let index = serve("/".parse::<Uri>().unwrap()).await;
        assert_eq!(index.status(), StatusCode::OK);
        assert_eq!(
            index.headers()[header::CONTENT_TYPE],
            "text/html; charset=utf-8"
        );
        let missing_api = serve("/api/not-a-real-endpoint".parse::<Uri>().unwrap()).await;
        assert_eq!(missing_api.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn missing_bundle_fails_closed_with_message() {
        for path in ["", "assets/app.js", "chat/abc"] {
            let response = serve_path(path, |_| None);
            assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE, "path {path:?}");
            let body = axum::body::to_bytes(response.into_body(), 8 * 1024)
                .await
                .unwrap();
            let text = std::str::from_utf8(&body).unwrap();
            assert!(text.contains("web_dist"), "message names bundle dir: {text:?}");
            assert_eq!(text, BUNDLE_MISSING_MESSAGE);
        }
    }

    #[test]
    fn traversal_rejected_before_bundle_gate() {
        assert_eq!(
            serve_path("../secret", |_| None).status(),
            StatusCode::BAD_REQUEST
        );
        assert_eq!(
            serve_path("a/../b", fixture).status(),
            StatusCode::BAD_REQUEST
        );
    }

    #[test]
    fn fixture_bundle_serves_entry_asset_spa_and_404() {
        let index = serve_path("", fixture);
        assert_eq!(index.status(), StatusCode::OK);
        assert_eq!(
            index.headers()[header::CONTENT_TYPE],
            "text/html; charset=utf-8"
        );
        assert_eq!(
            index.headers()[header::CACHE_CONTROL],
            "no-cache"
        );

        let asset = serve_path("assets/app.js", fixture);
        assert_eq!(asset.status(), StatusCode::OK);
        assert_eq!(
            asset.headers()[header::CONTENT_TYPE],
            "text/javascript; charset=utf-8"
        );
        assert_eq!(
            asset.headers()[header::CACHE_CONTROL],
            "public, max-age=31536000, immutable"
        );

        let spa = serve_path("chat/abc", fixture);
        assert_eq!(spa.status(), StatusCode::OK);
        assert_eq!(
            spa.headers()[header::CONTENT_TYPE],
            "text/html; charset=utf-8"
        );

        assert_eq!(
            serve_path("missing.js", fixture).status(),
            StatusCode::NOT_FOUND
        );
    }
}
