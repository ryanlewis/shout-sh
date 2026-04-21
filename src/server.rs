// shout.sh — curl-friendly ANSI banner service
// Copyright (C) 2026 Ryan Lewis
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

use std::convert::Infallible;
use std::sync::LazyLock;
use std::time::Duration;

use async_stream::stream;
use axum::Router;
use axum::body::Body;
use axum::extract::Path;
use axum::http::{HeaderMap, StatusCode, Uri, header};
use axum::response::{IntoResponse, Response};
use axum::routing::get;

use crate::fonts;
use crate::parser::{Mode, RenderConfig, parse};
use crate::render::{RenderError, banner, emit_shaded, render_cells, render_config};
use crate::sgr::{self, Cell};
use crate::shader::{Fire, Identity, Rainbow};

/// Help text is built once at startup; every `GET /` serves the same bytes.
static HELP: LazyLock<String> = LazyLock::new(build_help_text);

/// `/fonts` body: the canonical font list with a trailing newline.
static FONTS_BODY: LazyLock<String> = LazyLock::new(|| format!("{}\n", fonts::list_newline()));

pub fn app() -> Router {
    Router::new()
        .route("/", get(root))
        .route("/health", get(health))
        .route("/favicon.ico", get(favicon))
        .route("/fonts", get(fonts_list))
        .route("/fonts/{name}", get(font_preview))
        .fallback(render_fallback)
}

fn plain<S: Into<String>>(body: S) -> Response {
    (
        [(header::CONTENT_TYPE, "text/plain; charset=utf-8")],
        body.into(),
    )
        .into_response()
}

fn error_response(err: RenderError) -> Response {
    let status = match err {
        RenderError::EmptyText => StatusCode::OK,
        _ => StatusCode::BAD_REQUEST,
    };
    let body = format!("{}\n", err.message());
    (
        status,
        [(header::CONTENT_TYPE, "text/plain; charset=utf-8")],
        body,
    )
        .into_response()
}

async fn root() -> Response {
    plain(HELP.as_str())
}

async fn health() -> Response {
    plain("ok\n")
}

async fn favicon() -> Response {
    StatusCode::NO_CONTENT.into_response()
}

async fn fonts_list() -> Response {
    plain(FONTS_BODY.as_str())
}

async fn font_preview(Path(name): Path<String>) -> Response {
    let lower = name.to_lowercase();
    if !fonts::is_font(&lower) {
        return error_response(RenderError::UnknownFont);
    }
    let cfg = RenderConfig {
        text: "Hello World".into(),
        font: lower,
        ..Default::default()
    };
    match render_config(&cfg) {
        Ok(out) => plain(out),
        Err(e) => error_response(e),
    }
}

/// Browsers hitting a stream URL would hang a tab forever. Detect them by
/// Accept header or User-Agent prefix and force `once` → static frame.
fn is_browser(headers: &HeaderMap) -> bool {
    let accepts_html = headers
        .get(header::ACCEPT)
        .and_then(|v| v.to_str().ok())
        .is_some_and(|s| s.contains("text/html"));
    let ua_browser = headers
        .get(header::USER_AGENT)
        .and_then(|v| v.to_str().ok())
        .is_some_and(|s| s.starts_with("Mozilla/"));
    accepts_html || ua_browser
}

async fn render_fallback(uri: Uri, headers: HeaderMap) -> Response {
    let mut cfg = parse(uri.path(), uri.query());
    if is_browser(&headers) {
        cfg.once = true;
    }

    // JSON always returns a single static frame.
    if cfg.json {
        return static_response(&cfg);
    }

    if cfg.should_animate() {
        return match animated_response(&cfg) {
            Ok(r) => r,
            Err(e) => error_response(e),
        };
    }

    static_response(&cfg)
}

fn static_response(cfg: &RenderConfig) -> Response {
    match render_config(cfg) {
        Ok(out) if cfg.json => {
            let body = serde_json::json!({
                "text": cfg.text,
                "font": cfg.font,
                "render": out,
            });
            (
                [(header::CONTENT_TYPE, "application/json")],
                body.to_string(),
            )
                .into_response()
        }
        Ok(out) => plain(out),
        Err(e) => error_response(e),
    }
}

fn animated_response(cfg: &RenderConfig) -> Result<Response, RenderError> {
    let cells = render_cells(cfg)?;
    let rows = sgr::row_count(&cells);
    let mode = cfg.mode.unwrap_or(Mode::Solid);
    let fps = cfg.fps;
    let timeout = cfg.timeout;

    let stream = build_stream(cells, rows, mode, fps, timeout);
    let body = Body::from_stream(stream);

    Ok(Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "text/plain; charset=utf-8")
        .header(header::CACHE_CONTROL, "no-cache")
        .header("X-Content-Type-Options", "nosniff")
        .body(body)
        .expect("valid response"))
}

/// Build the frame stream. Each yielded chunk is a full frame (cursor-up
/// prelude + shaded cells). Stream ends after `timeout` seconds or when
/// the consumer drops it (client disconnect).
fn build_stream(
    cells: Vec<Cell>,
    rows: u16,
    mode: Mode,
    fps: u32,
    timeout: u32,
) -> impl futures_core::Stream<Item = Result<String, Infallible>> {
    stream! {
        // First chunk: hide cursor, write the initial frame in-place.
        let first = format!("\x1b[?25l{}", shade(&cells, mode, rows, 0));
        yield Ok(first);

        // 0-row guard — shouldn't happen, but \x1b[0A is invalid.
        let up = if rows > 0 {
            format!("\x1b[{rows}A\r")
        } else {
            String::from("\r")
        };

        let tick = Duration::from_millis((1000 / fps.max(1)) as u64);
        let mut iv = tokio::time::interval(tick);
        iv.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        // Consume the immediate first tick so the next frame waits one interval.
        iv.tick().await;

        let deadline = tokio::time::sleep(Duration::from_secs(timeout as u64));
        tokio::pin!(deadline);

        let mut frame: u64 = 1;
        loop {
            tokio::select! {
                _ = &mut deadline => break,
                _ = iv.tick() => {
                    let chunk = format!("{up}{}", shade(&cells, mode, rows, frame));
                    yield Ok(chunk);
                    frame += 1;
                }
            }
        }

        // Final chunk: reset SGR, show cursor, trailing newline.
        yield Ok(String::from("\x1b[0m\x1b[?25h\n"));
    }
}

fn shade(cells: &[Cell], mode: Mode, rows: u16, frame: u64) -> String {
    match mode {
        Mode::Rainbow => emit_shaded(cells, &Rainbow, frame),
        Mode::Fire => emit_shaded(cells, &Fire { rows }, frame),
        Mode::Solid => emit_shaded(cells, &Identity, frame),
    }
}

/// Exposed for tests that assert on help content.
pub fn help_text() -> String {
    HELP.clone()
}

fn build_help_text() -> String {
    let mut s = String::new();
    s.push_str(&banner());
    s.push('\n');
    s.push_str("curl-friendly ansi banners. shout text at your terminal.\n\n");
    s.push_str("USAGE\n");
    s.push_str("  $ curl shout.sh/{text}\n");
    s.push_str("  $ curl shout.sh/{directives}/{text}\n");
    s.push_str("  $ curl -N shout.sh/rainbow/hi    # -N: animation streams\n\n");
    s.push_str("  $ curl shout.sh/HELLO\n");
    s.push_str("  $ curl shout.sh/tiny/hello+world\n");
    s.push_str("  $ curl shout.sh/red/alert\n");
    s.push_str("  $ curl -N shout.sh/fire/boom\n");
    s.push_str("  $ curl 'shout.sh/HELLO?format=json'\n\n");
    s.push_str("FONTS\n");
    for f in fonts::FONTS {
        s.push_str("  ");
        s.push_str(f);
        s.push('\n');
    }
    s.push('\n');
    s.push_str("MODES\n");
    s.push_str("  solid     single color. pair with a color directive.\n");
    s.push_str("  rainbow   animated hsl hue ring. curl -N to see it.\n");
    s.push_str("  fire      animated red/orange/yellow flicker. curl -N.\n\n");
    s.push_str("ANIMATION\n");
    s.push_str("  animate      force animation on any mode.\n");
    s.push_str("  once         force a single static frame.\n");
    s.push_str("  ?fps=N       frames per second. default 10, capped at 30.\n");
    s.push_str("  ?timeout=N   seconds before server closes. default 60, max 300.\n\n");
    s.push_str("COLORS\n");
    s.push_str("  red, green, blue, yellow, cyan, magenta, white, gray\n");
    s.push_str("  `*bright` variants, e.g. `redbright`, `cyanbright`.\n\n");
    s.push_str("MORE\n");
    s.push_str("  $ curl shout.sh/fonts          # list fonts\n");
    s.push_str("  $ curl shout.sh/fonts/block    # preview one\n");
    s
}
