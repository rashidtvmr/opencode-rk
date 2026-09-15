//! Embedded production web client served by the native singleton daemon.

use axum::{
    body::Body,
    http::{header, StatusCode, Uri},
    response::{IntoResponse, Response},
};
use include_dir::{include_dir, Dir};

static WEB_DIST: Dir<'_> = include_dir!("$CARGO_MANIFEST_DIR/web_dist");

pub async fn serve(uri: Uri) -> Response {
    let path = uri.path().trim_start_matches('/');
    if path == "health" || path == "api" || path.starts_with("api/") {
        return StatusCode::NOT_FOUND.into_response();
    }
    if path.contains("..") || path.contains('\0') {
        return StatusCode::BAD_REQUEST.into_response();
    }

    let requested = if path.is_empty() { "index.html" } else { path };
    if let Some(file) = WEB_DIST.get_file(requested) {
        return asset_response(requested, file.contents());
    }

    if !requested
        .rsplit('/')
        .next()
        .is_some_and(|name| name.contains('.'))
    {
        if let Some(index) = WEB_DIST.get_file("index.html") {
            return asset_response("index.html", index.contents());
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
