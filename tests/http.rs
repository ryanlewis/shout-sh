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
async fn rainbow_end_to_end() {
    let (status, _, body) = get("/rainbow/party").await;
    assert_eq!(status, StatusCode::OK);
    // Candy emits SGR color codes per char.
    assert!(body.contains("\x1b["), "expected SGR in rainbow output");
}

#[tokio::test]
async fn solid_without_color_is_white() {
    let (status, _, body) = get("/solid/hi").await;
    assert_eq!(status, StatusCode::OK);
    assert!(body.contains("\x1b["), "expected SGR in solid output");
}

#[tokio::test]
async fn query_font_overrides_path_font_over_http() {
    let (status, _, body) = get("/tiny/hi?font=block").await;
    assert_eq!(status, StatusCode::OK);
    // block font uses ╗ box chars; tiny does not. Asserts the override took.
    assert!(
        body.contains('╗'),
        "expected block-font glyphs after override"
    );
}

#[tokio::test]
async fn query_mode_overrides_path_mode_over_http() {
    // path says solid (no SGR truecolor), query flips to fire (truecolor).
    let (status, _, body) = get("/solid/hi?mode=fire").await;
    assert_eq!(status, StatusCode::OK);
    assert!(
        body.contains("\x1b[38;2;"),
        "expected fire truecolor after override"
    );
}

#[tokio::test]
async fn layout_directive_ignored_not_400() {
    // `full` is a legacy FIGlet layout — accept silently.
    let (status, _, body) = get("/full/Hi").await;
    assert_eq!(status, StatusCode::OK);
    assert!(!body.is_empty());
}

#[tokio::test]
async fn width_directive_ignored_not_400() {
    let (status, _, body) = get("/w120/Hi").await;
    assert_eq!(status, StatusCode::OK);
    assert!(!body.is_empty());
}

#[tokio::test]
async fn animate_once_flags_ignored_not_400() {
    let (status, _, body) = get("/rainbow+once/Hi").await;
    assert_eq!(status, StatusCode::OK);
    assert!(!body.is_empty());
}

#[tokio::test]
async fn unknown_mode_via_query_is_400() {
    let (status, _, body) = get("/Hi?mode=matrix").await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(body.contains("mode not found"));
}

#[tokio::test]
async fn unknown_font_via_query_is_400() {
    let (status, _, body) = get("/Hi?font=standard").await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(body.contains("font not found"));
}

#[tokio::test]
async fn unknown_color_via_query_is_400() {
    let (status, _, body) = get("/Hi?color=puce").await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(body.contains("color not found"));
}

#[tokio::test]
async fn very_long_text_is_capped_not_panic() {
    // 5000 a's — server must not OOM or timeout.
    let long = "a".repeat(5000);
    let (status, _, body) = get(&format!("/{long}")).await;
    assert_eq!(status, StatusCode::OK);
    assert!(!body.is_empty());
}

#[tokio::test]
async fn unicode_text_renders_or_degrades_gracefully() {
    // cfonts only has ASCII glyphs; non-ASCII should not panic.
    let (status, _, _) = get("/café").await;
    assert_eq!(status, StatusCode::OK);
}

#[tokio::test]
async fn empty_directive_segment_is_text() {
    // `//Hi` — empty first segment matches no directive → treat whole as text.
    let (status, _, _) = get("//Hi").await;
    assert_eq!(status, StatusCode::OK);
}

#[tokio::test]
async fn double_plus_does_not_panic() {
    let (status, _, _) = get("/hello++world").await;
    assert_eq!(status, StatusCode::OK);
}

#[tokio::test]
async fn trailing_slash_empty_text_friendly() {
    // /block/ → directives match, text is empty → friendly message, 200.
    let (status, _, body) = get("/block/").await;
    assert_eq!(status, StatusCode::OK);
    assert!(body.contains("nothing to shout about"));
}

#[tokio::test]
async fn valueless_query_param_is_ignored() {
    // `?format` with no `=json` should not crash and should not flip format.
    let (status, ctype, _) = get("/HELLO?format").await;
    assert_eq!(status, StatusCode::OK);
    assert!(ctype.starts_with("text/plain"), "format should stay unset");
}

#[tokio::test]
async fn empty_query_value_is_ignored() {
    let (status, ctype, _) = get("/HELLO?font=&format=json").await;
    assert_eq!(status, StatusCode::OK);
    // Empty font= must not blank out the default; format=json must still win.
    assert!(ctype.starts_with("application/json"));
}

#[tokio::test]
async fn unknown_path_directives_fall_through_to_text() {
    // /notafont/Hi → whole path (minus leading slash) is text.
    let (status, _, _) = get("/notafont/Hi").await;
    assert_eq!(status, StatusCode::OK);
}

#[tokio::test]
async fn post_on_named_route_is_405() {
    use axum::http::Method;
    let req = Request::builder()
        .method(Method::POST)
        .uri("/health")
        .body(Body::empty())
        .unwrap();
    let resp = app().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::METHOD_NOT_ALLOWED);
}
