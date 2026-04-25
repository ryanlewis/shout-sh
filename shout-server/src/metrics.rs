// shout.sh — curl-friendly ANSI banner service
// Copyright (C) 2026 Ryan Lewis
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

//! Prometheus metrics. Scraped at `GET /__metrics`.
//!
//! Labels are kept low-cardinality on purpose: the `route` label is the
//! matched axum path (or `/render` for the catch-all banner handler),
//! never the raw URL.

use std::sync::LazyLock;
use std::time::Instant;

use axum::Router;
use axum::body::Body;
use axum::extract::{MatchedPath, Request};
use axum::http::{StatusCode, header};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use prometheus::{
    Encoder, HistogramVec, IntCounter, IntCounterVec, IntGauge, Registry, TextEncoder,
    register_histogram_vec_with_registry, register_int_counter_vec_with_registry,
    register_int_counter_with_registry, register_int_gauge_with_registry,
};
#[cfg(target_os = "linux")]
use prometheus::process_collector::ProcessCollector;

use shout_core::fonts;
use shout_core::parser::{Mode, RenderConfig};
use shout_core::presets;
use shout_core::render::RenderError;

static REGISTRY: LazyLock<Registry> = LazyLock::new(Registry::new);

static HTTP_REQUESTS: LazyLock<IntCounterVec> = LazyLock::new(|| {
    register_int_counter_vec_with_registry!(
        "shout_http_requests_total",
        "Total HTTP requests received.",
        &["route", "status"],
        REGISTRY
    )
    .expect("register shout_http_requests_total")
});

static HTTP_DURATION: LazyLock<HistogramVec> = LazyLock::new(|| {
    register_histogram_vec_with_registry!(
        "shout_http_request_duration_seconds",
        "HTTP request duration in seconds, from handler entry to response.",
        &["route"],
        vec![0.0005, 0.001, 0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0],
        REGISTRY
    )
    .expect("register shout_http_request_duration_seconds")
});

static RENDERS: LazyLock<IntCounterVec> = LazyLock::new(|| {
    register_int_counter_vec_with_registry!(
        "shout_renders_total",
        "Total banner renders served. `kind` is static|animated|json.",
        &["kind", "mode"],
        REGISTRY
    )
    .expect("register shout_renders_total")
});

static RENDER_ERRORS: LazyLock<IntCounterVec> = LazyLock::new(|| {
    register_int_counter_vec_with_registry!(
        "shout_render_errors_total",
        "Render errors returned to clients.",
        &["kind"],
        REGISTRY
    )
    .expect("register shout_render_errors_total")
});

static STREAMS_ACTIVE: LazyLock<IntGauge> = LazyLock::new(|| {
    register_int_gauge_with_registry!(
        "shout_streams_active",
        "Animated banner streams currently open.",
        REGISTRY
    )
    .expect("register shout_streams_active")
});

static STREAM_FRAMES: LazyLock<IntCounter> = LazyLock::new(|| {
    register_int_counter_with_registry!(
        "shout_stream_frames_total",
        "Animation frames emitted across all streams.",
        REGISTRY
    )
    .expect("register shout_stream_frames_total")
});

static FONT_REQUESTS: LazyLock<IntCounterVec> = LazyLock::new(|| {
    register_int_counter_vec_with_registry!(
        "shout_font_requests_total",
        "Renders grouped by font name. Cardinality is bounded by the embedded font list.",
        &["font"],
        REGISTRY
    )
    .expect("register shout_font_requests_total")
});

static PRESET_REQUESTS: LazyLock<IntCounterVec> = LazyLock::new(|| {
    register_int_counter_vec_with_registry!(
        "shout_preset_requests_total",
        "Renders grouped by preset name. Empty/invalid preset is reported as `none`.",
        &["preset"],
        REGISTRY
    )
    .expect("register shout_preset_requests_total")
});

static BUILD_INFO: LazyLock<IntGauge> = LazyLock::new(|| {
    let g = IntGauge::with_opts(
        prometheus::Opts::new("shout_build_info", "Build info, constant 1.")
            .const_label("version", env!("CARGO_PKG_VERSION")),
    )
    .expect("build_info opts");
    REGISTRY
        .register(Box::new(g.clone()))
        .expect("register shout_build_info");
    g.set(1);
    g
});

/// Touch every LazyLock so metrics show up in `/__metrics` even before
/// any request increments them.
pub fn init() {
    LazyLock::force(&HTTP_REQUESTS);
    LazyLock::force(&HTTP_DURATION);
    LazyLock::force(&RENDERS);
    LazyLock::force(&RENDER_ERRORS);
    LazyLock::force(&STREAMS_ACTIVE);
    LazyLock::force(&STREAM_FRAMES);
    LazyLock::force(&FONT_REQUESTS);
    LazyLock::force(&PRESET_REQUESTS);
    LazyLock::force(&BUILD_INFO);
    // Process-level metrics: process_cpu_seconds_total,
    // process_resident_memory_bytes, process_open_fds, process_threads, …
    // The collector reads /proc, so it's Linux-only.
    #[cfg(target_os = "linux")]
    REGISTRY
        .register(Box::new(ProcessCollector::for_self()))
        .expect("register process collector");
}

#[derive(Copy, Clone)]
pub enum RenderKind {
    Static,
    Animated,
    Json,
}

impl RenderKind {
    fn as_str(self) -> &'static str {
        match self {
            Self::Static => "static",
            Self::Animated => "animated",
            Self::Json => "json",
        }
    }
}

fn mode_label(mode: Option<Mode>) -> &'static str {
    match mode {
        None => "default",
        Some(Mode::Solid) => "solid",
        Some(Mode::Rainbow) => "rainbow",
        Some(Mode::Fire) => "fire",
    }
}

fn error_label(err: &RenderError) -> &'static str {
    match err {
        RenderError::UnknownFont => "unknown_font",
        RenderError::UnknownColor => "unknown_color",
        RenderError::UnknownPreset => "unknown_preset",
        RenderError::EmptyText => "empty_text",
    }
}

pub fn record_render(kind: RenderKind, cfg: &RenderConfig) {
    RENDERS
        .with_label_values(&[kind.as_str(), mode_label(cfg.mode)])
        .inc();
    // `fonts::is_font` / `presets::is_preset` guard against `?font=` and
    // `?preset=` query values that bypass parser validation — without
    // them, a bad client could explode the label cardinality.
    if fonts::is_font(&cfg.font) {
        FONT_REQUESTS.with_label_values(&[&cfg.font]).inc();
    }
    let preset = if presets::is_preset(&cfg.preset) {
        cfg.preset.as_str()
    } else {
        "none"
    };
    PRESET_REQUESTS.with_label_values(&[preset]).inc();
}

pub fn record_error(err: &RenderError) {
    RENDER_ERRORS.with_label_values(&[error_label(err)]).inc();
}

pub fn record_frame() {
    STREAM_FRAMES.inc();
}

/// RAII guard: increments `streams_active` on construction and decrements
/// on drop so we stay balanced even if the stream is dropped mid-frame
/// (client disconnect, timeout, anything).
pub struct StreamGuard;

impl StreamGuard {
    pub fn new() -> Self {
        STREAMS_ACTIVE.inc();
        Self
    }
}

impl Drop for StreamGuard {
    fn drop(&mut self) {
        STREAMS_ACTIVE.dec();
    }
}

/// Map a status code to a `&'static str` for the common cases so we skip
/// a per-request allocation. Uncommon codes fall through to `other`.
fn status_str(code: StatusCode) -> &'static str {
    match code.as_u16() {
        200 => "200",
        204 => "204",
        301 => "301",
        302 => "302",
        304 => "304",
        400 => "400",
        404 => "404",
        414 => "414",
        500 => "500",
        _ => "other",
    }
}

/// Axum middleware: record request count + duration keyed on the matched
/// route pattern. Unmatched paths (the banner fallback) collapse to
/// `/render` to cap cardinality.
pub async fn track(req: Request, next: Next) -> Response {
    // Clone the Arc-backed MatchedPath before `req` is consumed — cheap
    // refcount bump, no String allocation.
    let matched = req.extensions().get::<MatchedPath>().cloned();
    let start = Instant::now();
    let resp = next.run(req).await;
    let route = matched.as_ref().map(MatchedPath::as_str).unwrap_or("/render");
    let status = status_str(resp.status());
    HTTP_REQUESTS.with_label_values(&[route, status]).inc();
    HTTP_DURATION
        .with_label_values(&[route])
        .observe(start.elapsed().as_secs_f64());
    resp
}

/// Router for the metrics-only listener. Bind this to a private
/// interface (e.g. the Tailscale IP) so public ingress cannot reach it.
pub fn metrics_app() -> Router {
    Router::new().route("/__metrics", get(handler))
}

/// `GET /__metrics` — Prometheus text exposition.
async fn handler() -> Response {
    let encoder = TextEncoder::new();
    let mut buf = Vec::with_capacity(4096);
    if let Err(e) = encoder.encode(&REGISTRY.gather(), &mut buf) {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            [(header::CONTENT_TYPE, "text/plain; charset=utf-8")],
            format!("metrics encode error: {e}\n"),
        )
            .into_response();
    }
    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, encoder.format_type())
        .body(Body::from(buf))
        .expect("valid response")
}
