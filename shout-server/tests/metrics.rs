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

//! The metrics listener is optional: a bad `METRICS_ADDR` or a failed bind
//! must surface as an error value the binary can log and move past, never
//! as a panic or process exit.

use axum::body::{Body, to_bytes};
use axum::http::{Request, StatusCode};
use tower::ServiceExt;

use shout_server::{MetricsListenError, app, bind_metrics, metrics_app};

#[tokio::test]
async fn unparseable_addr_is_a_parse_error() {
    let err = bind_metrics("not-an-address").await.err().unwrap();
    assert!(matches!(err, MetricsListenError::Parse { .. }), "{err:?}");
    let msg = err.to_string();
    assert!(
        msg.starts_with("METRICS_ADDR=not-an-address invalid:"),
        "{msg}"
    );
}

#[tokio::test]
async fn already_bound_port_is_a_bind_error() {
    // Hold the port ourselves so the second bind is guaranteed to fail.
    let holder = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = holder.local_addr().unwrap();

    let err = bind_metrics(&addr.to_string()).await.err().unwrap();
    assert!(matches!(err, MetricsListenError::Bind { .. }), "{err:?}");
    let msg = err.to_string();
    assert!(
        msg.starts_with(&format!("bind metrics {addr} failed:")),
        "{msg}"
    );
    drop(holder);
}

#[tokio::test]
async fn free_addr_binds_and_serves_metrics() {
    let listener = bind_metrics("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    assert_eq!(addr.ip(), std::net::Ipv4Addr::LOCALHOST);
    assert_ne!(addr.port(), 0);
    drop(listener);

    // The router itself is what the listener serves; check it end to end.
    // Building the main app is what registers the collectors (as the
    // binary does), so do that first or the registry is empty.
    let _ = app();
    let req = Request::builder()
        .uri("/__metrics")
        .body(Body::empty())
        .unwrap();
    let resp = metrics_app().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = to_bytes(resp.into_body(), 1024 * 1024).await.unwrap();
    let body = String::from_utf8_lossy(&bytes);
    assert!(body.contains("shout_build_info"), "{body}");
}
