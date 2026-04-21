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

use std::sync::LazyLock;

use axum::Router;
use axum::extract::Path;
use axum::http::{StatusCode, Uri, header};
use axum::response::{IntoResponse, Response};
use axum::routing::get;

use crate::fonts;
use crate::parser::{RenderConfig, parse};
use crate::render::{RenderError, banner, render_config};

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

async fn render_fallback(uri: Uri) -> Response {
    let cfg = parse(uri.path(), uri.query());
    let json = cfg.json;

    match render_config(&cfg) {
        Ok(out) if json => {
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
    s.push_str("  $ curl shout.sh/{directives}/{text}\n\n");
    s.push_str("  $ curl shout.sh/HELLO\n");
    s.push_str("  $ curl shout.sh/tiny/hello+world\n");
    s.push_str("  $ curl shout.sh/red/alert\n");
    s.push_str("  $ curl shout.sh/fire/boom\n");
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
    s.push_str("  rainbow   per-char bright palette.\n");
    s.push_str("  fire      warm gradient: red -> orange -> yellow.\n\n");
    s.push_str("COLORS\n");
    s.push_str("  red, green, blue, yellow, cyan, magenta, white, gray\n");
    s.push_str("  `*bright` variants, e.g. `redbright`, `cyanbright`.\n\n");
    s.push_str("MORE\n");
    s.push_str("  $ curl shout.sh/fonts          # list fonts\n");
    s.push_str("  $ curl shout.sh/fonts/block    # preview one\n");
    s
}
