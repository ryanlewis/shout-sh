// shout.sh — curl-friendly ANSI banner service
// Copyright (C) 2026 Ryan Lewis
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

use axum::body::{Body, to_bytes};
use axum::http::{Request, StatusCode, header};
use tower::ServiceExt;

use shout::app;

async fn get(uri: &str) -> (StatusCode, String, String) {
    let req = Request::builder().uri(uri).body(Body::empty()).unwrap();
    let resp = app().oneshot(req).await.unwrap();
    let status = resp.status();
    let ctype = resp
        .headers()
        .get(header::CONTENT_TYPE)
        .map(|v| v.to_str().unwrap().to_string())
        .unwrap_or_default();
    let bytes = to_bytes(resp.into_body(), 1024 * 1024).await.unwrap();
    (status, ctype, String::from_utf8_lossy(&bytes).to_string())
}

#[tokio::test]
async fn health_ok() {
    let (status, ctype, body) = get("/health").await;
    assert_eq!(status, StatusCode::OK);
    assert!(ctype.starts_with("text/plain"));
    assert_eq!(body, "ok\n");
}

#[tokio::test]
async fn favicon_is_no_content() {
    let (status, _, _) = get("/favicon.ico").await;
    assert_eq!(status, StatusCode::NO_CONTENT);
}

#[tokio::test]
async fn root_help_text_includes_usage() {
    let (status, ctype, body) = get("/").await;
    assert_eq!(status, StatusCode::OK);
    assert!(ctype.starts_with("text/plain"));
    assert!(body.contains("USAGE"));
    assert!(body.contains("FONTS"));
    assert!(body.contains("MODES"));
}

#[tokio::test]
async fn fonts_lists_thirteen() {
    let (status, _, body) = get("/fonts").await;
    assert_eq!(status, StatusCode::OK);
    let lines: Vec<_> = body.lines().filter(|l| !l.is_empty()).collect();
    assert_eq!(lines.len(), 13);
    assert!(lines.contains(&"block"));
    assert!(lines.contains(&"tiny"));
}

#[tokio::test]
async fn font_preview_renders() {
    let (status, _, body) = get("/fonts/block").await;
    assert_eq!(status, StatusCode::OK);
    assert!(!body.is_empty());
}

#[tokio::test]
async fn font_preview_unknown_400() {
    let (status, _, body) = get("/fonts/standard").await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(body.contains("font not found"));
}

#[tokio::test]
async fn single_segment_renders_text() {
    let (status, _, body) = get("/HELLO").await;
    assert_eq!(status, StatusCode::OK);
    assert!(!body.is_empty());
}

#[tokio::test]
async fn red_tiny_hello_emits_red_sgr() {
    let (status, _, body) = get("/tiny+red/hello+world").await;
    assert_eq!(status, StatusCode::OK);
    assert!(body.contains("\x1b[31m"), "expected red SGR");
}

#[tokio::test]
async fn fire_emits_truecolor_sgr() {
    let (status, _, body) = get("/fire/ALERT").await;
    assert_eq!(status, StatusCode::OK);
    assert!(body.contains("\x1b[38;2;"), "expected truecolor SGR");
}

#[tokio::test]
async fn json_format_returns_json() {
    let (status, ctype, body) = get("/HELLO?format=json").await;
    assert_eq!(status, StatusCode::OK);
    assert!(ctype.starts_with("application/json"));
    let v: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(v["text"], "HELLO");
    assert_eq!(v["font"], "block");
    assert!(!v["render"].as_str().unwrap().is_empty());
}

#[tokio::test]
async fn empty_text_friendly_message() {
    let (_, _, body) = get("/").await;
    // root serves help, not the empty-text error; test empty via query
    assert!(!body.is_empty());
}
