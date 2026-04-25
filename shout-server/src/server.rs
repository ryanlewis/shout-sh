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

use shout_core::fonts;
use shout_core::parser::{MAX_PARAM_LEN, MAX_URL_LEN, Mode, RenderConfig, parse};
use shout_core::presets;
use shout_core::render::{RenderError, banner, emit_shaded, render_cells, render_config};
use shout_core::sgr::{self, Cell, ansi};
use shout_core::shader::{Filter, Fire, Identity, Rainbow};

use crate::metrics;

/// Help text is built once at startup; every `GET /` serves the same bytes.
static HELP: LazyLock<String> = LazyLock::new(build_help_text);

/// `/fonts` body: the canonical font list with a trailing newline.
static FONTS_BODY: LazyLock<String> = LazyLock::new(|| format!("{}\n", fonts::list_newline()));

static PRESETS_BODY: LazyLock<String> = LazyLock::new(|| format!("{}\n", presets::list_newline()));

pub fn app() -> Router {
    metrics::init();
    Router::new()
        .route("/", get(root))
        .route("/health", get(health))
        .route("/favicon.ico", get(favicon))
        .route("/favicon.svg", get(favicon_svg))
        .route("/og.png", get(og_image))
        .route("/fonts", get(fonts_list))
        .route("/fonts/{name}", get(font_preview))
        .route("/presets", get(presets_list))
        .route("/presets/{name}", get(preset_preview))
        .route("/_app/{file}", get(app_asset))
        .fallback(render_fallback)
        .layer(axum::middleware::from_fn(metrics::track))
}

/// Embedded playground assets. Keep this list in sync with web/dist/.
/// Anything here is served at `/_app/{file}` with its matching MIME.
const ASSET_INDEX_HTML: &[u8] = include_bytes!("../../web/dist/index.html");
const ASSET_PRIVACY_HTML: &[u8] = include_bytes!("../../web/dist/privacy.html");
const ASSET_ABOUT_HTML: &[u8] = include_bytes!("../../web/dist/about.html");
const ASSET_MAIN_JS: &[u8] = include_bytes!("../../web/dist/main.js");
const ASSET_MAIN_CSS: &[u8] = include_bytes!("../../web/dist/main.css");
const ASSET_WASM_JS: &[u8] = include_bytes!("../../web/dist/shout_wasm.js");
const ASSET_WASM_BG: &[u8] = include_bytes!("../../web/dist/shout_wasm_bg.wasm");
const ASSET_FAVICON_SVG: &[u8] = include_bytes!("../../web/dist/favicon.svg");
const ASSET_OG_PNG: &[u8] = include_bytes!("../../web/dist/og.png");

fn asset_for(name: &str) -> Option<(&'static [u8], &'static str)> {
    Some(match name {
        "index.html" => (ASSET_INDEX_HTML, "text/html; charset=utf-8"),
        "main.js" => (ASSET_MAIN_JS, "text/javascript; charset=utf-8"),
        "main.css" => (ASSET_MAIN_CSS, "text/css; charset=utf-8"),
        "shout_wasm.js" => (ASSET_WASM_JS, "text/javascript; charset=utf-8"),
        "shout_wasm_bg.wasm" => (ASSET_WASM_BG, "application/wasm"),
        _ => return None,
    })
}

async fn app_asset(Path(file): Path<String>) -> Response {
    if file.len() > MAX_PARAM_LEN {
        return StatusCode::NOT_FOUND.into_response();
    }
    match asset_for(&file) {
        Some((body, ctype)) => (
            [
                (header::CONTENT_TYPE, ctype),
                (header::CACHE_CONTROL, "no-cache"),
            ],
            body,
        )
            .into_response(),
        None => StatusCode::NOT_FOUND.into_response(),
    }
}

fn plain<S: Into<String>>(body: S) -> Response {
    (
        [(header::CONTENT_TYPE, "text/plain; charset=utf-8")],
        body.into(),
    )
        .into_response()
}

fn error_response(err: RenderError) -> Response {
    metrics::record_error(&err);
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

async fn root(headers: HeaderMap) -> Response {
    if accepts_html(&headers) {
        return (
            [(header::CONTENT_TYPE, "text/html; charset=utf-8")],
            ASSET_INDEX_HTML,
        )
            .into_response();
    }
    plain(HELP.as_str())
}

fn accepts_html(headers: &HeaderMap) -> bool {
    headers
        .get(header::ACCEPT)
        .and_then(|v| v.to_str().ok())
        .is_some_and(|s| s.contains("text/html"))
}

async fn health() -> Response {
    plain("ok\n")
}

async fn favicon() -> Response {
    StatusCode::NO_CONTENT.into_response()
}

async fn favicon_svg() -> Response {
    (
        [
            (header::CONTENT_TYPE, "image/svg+xml"),
            (header::CACHE_CONTROL, "public, max-age=86400"),
        ],
        ASSET_FAVICON_SVG,
    )
        .into_response()
}

async fn og_image() -> Response {
    (
        [
            (header::CONTENT_TYPE, "image/png"),
            (header::CACHE_CONTROL, "public, max-age=86400"),
        ],
        ASSET_OG_PNG,
    )
        .into_response()
}

async fn fonts_list() -> Response {
    plain(FONTS_BODY.as_str())
}

async fn font_preview(Path(name): Path<String>) -> Response {
    if name.len() > MAX_PARAM_LEN {
        return error_response(RenderError::UnknownFont);
    }
    let lower = name.to_lowercase();
    if !fonts::is_font(&lower) {
        return error_response(RenderError::UnknownFont);
    }
    // Preview with `sunset` so multi-color fonts show off their layers.
    let cfg = RenderConfig {
        text: "Hello World".into(),
        font: lower,
        preset: "sunset".into(),
        ..Default::default()
    };
    match render_config(&cfg) {
        Ok(out) => plain(out),
        Err(e) => error_response(e),
    }
}

async fn presets_list() -> Response {
    plain(PRESETS_BODY.as_str())
}

async fn preset_preview(Path(name): Path<String>) -> Response {
    if name.len() > MAX_PARAM_LEN {
        return error_response(RenderError::UnknownPreset);
    }
    let cfg = RenderConfig {
        text: "Hello World".into(),
        preset: name.to_lowercase(),
        ..Default::default()
    };
    match render_config(&cfg) {
        Ok(out) => plain(out),
        Err(e) => error_response(e),
    }
}

/// Paths that serve an HTML page to browsers but shout their name at curl.
fn browser_page(path: &str) -> Option<&'static [u8]> {
    match path {
        "/about" => Some(ASSET_ABOUT_HTML),
        "/privacy" => Some(ASSET_PRIVACY_HTML),
        _ => None,
    }
}

/// Browsers hitting a stream URL would hang a tab forever. Detect them by
/// Accept header or User-Agent prefix and force `once` → static frame.
fn is_browser(headers: &HeaderMap) -> bool {
    let ua_browser = headers
        .get(header::USER_AGENT)
        .and_then(|v| v.to_str().ok())
        .is_some_and(|s| s.starts_with("Mozilla/"));
    accepts_html(headers) || ua_browser
}

async fn render_fallback(uri: Uri, headers: HeaderMap) -> Response {
    let path_len = uri.path().len();
    let query_len = uri.query().map(|q| q.len() + 1).unwrap_or(0);
    if path_len + query_len > MAX_URL_LEN {
        return (
            StatusCode::URI_TOO_LONG,
            [(header::CONTENT_TYPE, "text/plain; charset=utf-8")],
            "url too long.\n",
        )
            .into_response();
    }
    let browser = is_browser(&headers);
    if browser {
        if let Some(asset) = browser_page(uri.path()) {
            return (
                [(header::CONTENT_TYPE, "text/html; charset=utf-8")],
                asset,
            )
                .into_response();
        }
    }
    let mut cfg = parse(uri.path(), uri.query());
    if browser {
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
            metrics::record_render(metrics::RenderKind::Json, cfg);
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
        Ok(out) => {
            metrics::record_render(metrics::RenderKind::Static, cfg);
            plain(out)
        }
        Err(e) => error_response(e),
    }
}

fn animated_response(cfg: &RenderConfig) -> Result<Response, RenderError> {
    let cells = render_cells(cfg)?;
    let rows = sgr::row_count(&cells);
    let shader = Shader::for_mode(cfg.mode.unwrap_or(Mode::Solid), rows);
    metrics::record_render(metrics::RenderKind::Animated, cfg);
    let stream = build_stream(cells, rows, shader, cfg.fps, cfg.timeout);

    Ok(Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "text/plain; charset=utf-8")
        .header(header::CACHE_CONTROL, "no-cache")
        .header("X-Content-Type-Options", "nosniff")
        .body(Body::from_stream(stream))
        .expect("valid response"))
}

/// Static dispatch over the three concrete filters so the per-cell
/// `shade` call in the hot path stays devirtualized.
enum Shader {
    Rainbow,
    Fire(Fire),
    Identity,
}

impl Shader {
    fn for_mode(mode: Mode, rows: u16) -> Self {
        match mode {
            Mode::Rainbow => Self::Rainbow,
            Mode::Fire => Self::Fire(Fire { rows }),
            Mode::Solid => Self::Identity,
        }
    }
}

impl Filter for Shader {
    fn shade(&self, cell: &Cell, frame: u64) -> Option<sgr::Rgb> {
        match self {
            Self::Rainbow => Rainbow.shade(cell, frame),
            Self::Fire(f) => f.shade(cell, frame),
            Self::Identity => Identity.shade(cell, frame),
        }
    }
}

fn build_stream(
    cells: Vec<Cell>,
    rows: u16,
    shader: Shader,
    fps: u32,
    timeout: u32,
) -> impl futures_core::Stream<Item = Result<String, Infallible>> {
    stream! {
        let _guard = metrics::StreamGuard::new();
        let first = format!(
            "{}{}{}{}",
            ansi::HIDE_CURSOR, ansi::CLEAR_SCREEN, ansi::CURSOR_HOME,
            emit_shaded(&cells, &shader, 0),
        );
        metrics::record_frame();
        yield Ok(first);

        // \x1b[0A is invalid; skip cursor-up if the banner had no rows.
        let up = if rows > 0 {
            format!("\x1b[{rows}A\r")
        } else {
            String::from("\r")
        };

        let tick = Duration::from_millis((1000 / fps.max(1)) as u64);
        let mut iv = tokio::time::interval(tick);
        iv.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        iv.tick().await;

        let deadline = tokio::time::sleep(Duration::from_secs(timeout as u64));
        tokio::pin!(deadline);

        let mut frame: u64 = 1;
        loop {
            tokio::select! {
                _ = &mut deadline => break,
                _ = iv.tick() => {
                    metrics::record_frame();
                    yield Ok(format!("{up}{}", emit_shaded(&cells, &shader, frame)));
                    frame += 1;
                }
            }
        }

        yield Ok(format!("{}{}\n", ansi::SGR_RESET, ansi::SHOW_CURSOR));
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
    s.push_str("PRESETS\n");
    s.push_str("  curated two-color palettes. multi-color fonts use both layers;\n");
    s.push_str("  single-color fonts keep just the first.\n  ");
    for (i, p) in presets::PRESETS.iter().enumerate() {
        if i > 0 {
            s.push_str(", ");
        }
        s.push_str(p.name);
    }
    s.push_str("\n\n");
    s.push_str("MORE\n");
    s.push_str("  $ curl shout.sh/fonts            # list fonts\n");
    s.push_str("  $ curl shout.sh/fonts/block      # preview one\n");
    s.push_str("  $ curl shout.sh/presets          # list presets\n");
    s.push_str("  $ curl shout.sh/presets/sunset   # preview one\n");
    s
}
