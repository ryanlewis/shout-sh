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

use shout_server::app;

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
async fn presets_lists_all() {
    let (status, _, body) = get("/presets").await;
    assert_eq!(status, StatusCode::OK);
    let lines: Vec<_> = body.lines().filter(|l| !l.is_empty()).collect();
    assert!(lines.contains(&"sunset"));
    assert!(lines.contains(&"ocean"));
}

#[tokio::test]
async fn preset_preview_renders() {
    let (status, _, body) = get("/presets/sunset").await;
    assert_eq!(status, StatusCode::OK);
    assert!(body.contains("\x1b[38;2;"));
}

#[tokio::test]
async fn preset_preview_unknown_400() {
    let (status, _, body) = get("/presets/puce").await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(body.contains("preset not found"));
}

#[tokio::test]
async fn preset_directive_renders_truecolor() {
    let (status, _, body) = get("/sunset/hi").await;
    assert_eq!(status, StatusCode::OK);
    assert!(body.contains("\x1b[38;2;"));
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
async fn fire_once_emits_truecolor_sgr() {
    // Without `once`, /fire/ALERT streams — phase-2 default-animate for fire.
    let (status, _, body) = get("/fire+once/ALERT").await;
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
async fn rainbow_once_end_to_end() {
    // Phase-2: rainbow animates by default; `once` forces a static frame.
    let (status, _, body) = get("/rainbow+once/party").await;
    assert_eq!(status, StatusCode::OK);
    // HSL shader emits truecolor SGR.
    assert!(
        body.contains("\x1b[38;2;"),
        "expected truecolor SGR in rainbow output"
    );
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
    // Append `&once` so the assertion reads a single static frame.
    let (status, _, body) = get("/solid+once/hi?mode=fire").await;
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
async fn unknown_mode_via_query_is_silently_ignored() {
    // Mirrors path-level behavior: unknown mode tokens don't 400, they
    // fall through to the default (no mode).
    let (status, ctype, _) = get("/Hi?mode=matrix").await;
    assert_eq!(status, StatusCode::OK);
    assert!(ctype.starts_with("text/plain"));
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
async fn very_long_text_within_cap_renders_ok() {
    // Just under MAX_URL_LEN — server truncates text to MAX_TEXT_LEN, no panic.
    let long = "a".repeat(200);
    let (status, _, body) = get(&format!("/{long}")).await;
    assert_eq!(status, StatusCode::OK);
    assert!(!body.is_empty());
}

#[tokio::test]
async fn oversize_url_rejected_with_414() {
    // Past MAX_URL_LEN we refuse outright rather than alloc / decode it.
    let huge = "a".repeat(1000);
    let (status, _, _) = get(&format!("/{huge}")).await;
    assert_eq!(status, StatusCode::URI_TOO_LONG);
}

#[tokio::test]
async fn escape_sequences_in_text_are_stripped() {
    // %1B is literal ESC. If this leaked into the banner body it would let a
    // caller smuggle terminal-hijack sequences through shout.sh.
    let (status, _, body) = get("/%1B%5B31mPWNED").await;
    assert_eq!(status, StatusCode::OK);
    // The only ESC in the body should be the SGR reset at the end of the
    // banner — never the raw `%1B[31m` we asked for.
    assert!(!body.contains("\x1b[31mPWNED"));
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

async fn get_with_accept(uri: &str, accept: &str) -> (StatusCode, String, Vec<u8>) {
    let req = Request::builder()
        .uri(uri)
        .header(header::ACCEPT, accept)
        .body(Body::empty())
        .unwrap();
    let resp = app().oneshot(req).await.unwrap();
    let status = resp.status();
    let ctype = resp
        .headers()
        .get(header::CONTENT_TYPE)
        .map(|v| v.to_str().unwrap().to_string())
        .unwrap_or_default();
    let bytes = to_bytes(resp.into_body(), 4 * 1024 * 1024).await.unwrap();
    (status, ctype, bytes.to_vec())
}

#[tokio::test]
async fn root_html_accept_returns_playground() {
    let (status, ctype, body) = get_with_accept("/", "text/html").await;
    assert_eq!(status, StatusCode::OK);
    assert!(ctype.starts_with("text/html"));
    let s = String::from_utf8_lossy(&body);
    assert!(s.contains("<title>shout.sh"), "expected playground HTML");
}

#[tokio::test]
async fn root_plain_accept_returns_help() {
    let (status, ctype, body) = get_with_accept("/", "text/plain").await;
    assert_eq!(status, StatusCode::OK);
    assert!(ctype.starts_with("text/plain"));
    let s = String::from_utf8_lossy(&body);
    assert!(s.contains("USAGE"));
}

#[tokio::test]
async fn app_asset_main_js_has_javascript_ctype() {
    let (status, ctype, body) = get("/_app/main.js").await;
    assert_eq!(status, StatusCode::OK);
    assert!(ctype.contains("javascript"), "got: {ctype}");
    assert!(!body.is_empty());
}

#[tokio::test]
async fn app_asset_main_css_has_css_ctype() {
    let (status, ctype, _) = get("/_app/main.css").await;
    assert_eq!(status, StatusCode::OK);
    assert!(ctype.contains("text/css"));
}

#[tokio::test]
async fn app_asset_wasm_has_application_wasm_ctype() {
    // Required for WebAssembly.instantiateStreaming to succeed in browsers.
    let (status, ctype, body) = get_with_accept("/_app/shout_wasm_bg.wasm", "*/*").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(ctype, "application/wasm", "got: {ctype}");
    // wasm magic: \0asm
    assert_eq!(&body[..4], b"\0asm");
}

#[tokio::test]
async fn app_asset_unknown_is_404() {
    let (status, _, _) = get("/_app/does-not-exist.js").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
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
