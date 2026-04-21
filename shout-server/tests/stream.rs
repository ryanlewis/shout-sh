// shout.sh — curl-friendly ANSI banner service
// Copyright (C) 2026 Ryan Lewis
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

use std::time::Duration;

use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use http_body_util::BodyExt;
use tokio::time::timeout;
use tower::ServiceExt;

use shout_server::app;

async fn stream_request(uri: &str) -> axum::response::Response {
    let req = Request::builder().uri(uri).body(Body::empty()).unwrap();
    app().oneshot(req).await.unwrap()
}

/// Read the next data frame with a hard deadline so a bug can't hang the
/// test runner. Returns the frame as a `String`.
async fn next_chunk(body: &mut Body, within: Duration) -> String {
    let frame = timeout(within, body.frame())
        .await
        .expect("timed out waiting for chunk")
        .expect("stream ended unexpectedly")
        .expect("stream error");
    let data = frame.into_data().expect("trailer instead of data");
    String::from_utf8_lossy(&data).to_string()
}

#[tokio::test]
async fn rainbow_streams_frames_with_cursor_controls() {
    let resp = stream_request("/rainbow/hi").await;
    assert_eq!(resp.status(), StatusCode::OK);
    let ctype = resp.headers().get(header::CONTENT_TYPE).unwrap();
    assert!(ctype.to_str().unwrap().starts_with("text/plain"));
    assert_eq!(
        resp.headers().get("x-content-type-options").unwrap(),
        "nosniff"
    );

    let mut body = resp.into_body();
    let c1 = next_chunk(&mut body, Duration::from_secs(2)).await;
    let c2 = next_chunk(&mut body, Duration::from_secs(2)).await;

    assert!(c1.starts_with("\x1b[?25l"), "first chunk must hide cursor");
    // second chunk begins with cursor-up-N then \r
    assert!(
        c2.contains("\x1b[") && c2.contains("A\r"),
        "second chunk should overwrite via cursor-up, got {:?}",
        &c2[..c2.len().min(20)]
    );
    // truecolor SGR present in both
    assert!(c1.contains("\x1b[38;2;"));
    assert!(c2.contains("\x1b[38;2;"));
    // frames differ (rainbow animates)
    assert_ne!(c1, c2);
}

#[tokio::test]
async fn rainbow_once_returns_single_static_chunk() {
    let resp = stream_request("/rainbow+once/hi").await;
    assert_eq!(resp.status(), StatusCode::OK);
    let body = resp.into_body();
    let bytes = http_body_util::BodyExt::collect(body)
        .await
        .unwrap()
        .to_bytes();
    let s = String::from_utf8_lossy(&bytes);
    assert!(
        !s.contains("\x1b[?25l"),
        "static response must not hide cursor"
    );
    assert!(!s.contains("A\r"), "static response must not use cursor-up");
    assert!(s.contains("\x1b[38;2;"), "expected truecolor SGR");
}

#[tokio::test]
async fn fire_stream_completes_and_resets() {
    // fps=30, timeout=1 → stream ends within ~1.1s; final chunk resets SGR.
    let resp = stream_request("/fire/alert?fps=30&timeout=1").await;
    assert_eq!(resp.status(), StatusCode::OK);

    let body = http_body_util::BodyExt::collect(resp.into_body());
    let collected = timeout(Duration::from_secs(3), body)
        .await
        .expect("stream did not finish within deadline")
        .unwrap()
        .to_bytes();
    let s = String::from_utf8_lossy(&collected);
    assert!(
        s.ends_with("\x1b[0m\x1b[?25h\n"),
        "stream must end with SGR/cursor reset"
    );
    assert!(
        s.starts_with("\x1b[?25l"),
        "stream must start with cursor hide"
    );
}

#[tokio::test]
async fn browser_accept_html_forces_static() {
    let req = Request::builder()
        .uri("/rainbow/hi")
        .header(header::ACCEPT, "text/html,application/xhtml+xml")
        .body(Body::empty())
        .unwrap();
    let resp = app().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = http_body_util::BodyExt::collect(resp.into_body())
        .await
        .unwrap()
        .to_bytes();
    let s = String::from_utf8_lossy(&bytes);
    assert!(!s.contains("\x1b[?25l"), "browser must get static response");
    assert!(s.contains("\x1b[38;2;"), "expected truecolor SGR");
}

#[tokio::test]
async fn browser_user_agent_forces_static() {
    let req = Request::builder()
        .uri("/rainbow/hi")
        .header(header::USER_AGENT, "Mozilla/5.0 (Macintosh)")
        .body(Body::empty())
        .unwrap();
    let resp = app().oneshot(req).await.unwrap();
    let bytes = http_body_util::BodyExt::collect(resp.into_body())
        .await
        .unwrap()
        .to_bytes();
    let s = String::from_utf8_lossy(&bytes);
    assert!(!s.contains("\x1b[?25l"));
}

#[tokio::test]
async fn json_on_animated_mode_is_static_json() {
    let resp = stream_request("/rainbow/hi?format=json").await;
    assert_eq!(resp.status(), StatusCode::OK);
    let ctype = resp.headers().get(header::CONTENT_TYPE).unwrap();
    assert!(ctype.to_str().unwrap().starts_with("application/json"));
    let bytes = http_body_util::BodyExt::collect(resp.into_body())
        .await
        .unwrap()
        .to_bytes();
    let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(v["text"], "hi");
    assert!(v["render"].as_str().unwrap().contains("\x1b[38;2;"));
}

#[tokio::test]
async fn fps_clamped_bounds_frame_count() {
    // ?fps=9999 clamps to 30; ?timeout=1 → ~30 frames in 1s.
    // Count chunks until stream closes, with a hard outer deadline.
    let resp = stream_request("/rainbow/hi?fps=9999&timeout=1").await;
    let mut body = resp.into_body();
    let mut chunks = 0u32;
    let deadline = Duration::from_secs(3);
    timeout(deadline, async {
        while let Some(Ok(_)) = body.frame().await {
            chunks += 1;
        }
    })
    .await
    .expect("stream did not terminate in time");
    // At 30fps over ~1s: ~30 frame chunks + 1 init + 1 final ≈ 32. Allow a wide band.
    assert!(chunks >= 10, "expected several frames, got {chunks}");
    assert!(chunks <= 80, "fps was not clamped, got {chunks}");
}

#[tokio::test]
async fn timeout_zero_clamps_to_default_not_hang() {
    // ?timeout=0 → clamped to default 60, but we cancel by dropping after 1 chunk.
    let resp = stream_request("/rainbow/hi?timeout=0&fps=30").await;
    let mut body = resp.into_body();
    // just read one chunk; dropping body cancels the stream.
    let _ = next_chunk(&mut body, Duration::from_secs(2)).await;
}

#[tokio::test]
async fn solid_never_animates() {
    // /solid/hi must return immediately as a static response.
    let resp = stream_request("/solid/hi").await;
    let bytes = timeout(
        Duration::from_secs(2),
        http_body_util::BodyExt::collect(resp.into_body()),
    )
    .await
    .expect("solid hung")
    .unwrap()
    .to_bytes();
    let s = String::from_utf8_lossy(&bytes);
    assert!(!s.contains("\x1b[?25l"));
}
