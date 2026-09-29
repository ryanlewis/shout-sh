// shout.sh — curl-friendly ANSI banner service
// Copyright (C) 2026 Ryan Lewis
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

//! Routing and response building, as plain Rust. The Worker glue turns a
//! `Reply` into a `worker::Response`: it fetches `Body::Asset` from the
//! ASSETS binding and drives `Body::Stream` on a timer.

use std::borrow::Cow;
use std::sync::LazyLock;

use percent_encoding::percent_decode_str;

use shout_core::fonts;
use shout_core::parser::{MAX_PARAM_LEN, MAX_URL_LEN, RenderConfig, parse};
use shout_core::presets;
use shout_core::render::{RenderError, banner, render_config};

use crate::event::{Event, ROUTE_RENDER, RenderKind};
use crate::stream::Animation;

/// Help text is built once per isolate; every `GET /` serves the same bytes.
static HELP: LazyLock<String> = LazyLock::new(build_help_text);

/// `/fonts` body: the canonical font list with a trailing newline.
static FONTS_BODY: LazyLock<String> = LazyLock::new(|| format!("{}\n", fonts::list_newline()));

static PRESETS_BODY: LazyLock<String> = LazyLock::new(|| format!("{}\n", presets::list_newline()));

pub const TEXT_PLAIN: &str = "text/plain; charset=utf-8";
const TEXT_HTML: &str = "text/html; charset=utf-8";
// Safe to cache forever because every /_app/* URL is content-hashed.
pub const IMMUTABLE_CACHE: &str = "public, max-age=31536000, immutable";
// HTML must not outlive a deploy: it embeds the hashed asset URLs and
// would otherwise reference 404'd files after a redeploy.
pub const HTML_CACHE: &str = "no-cache";
pub const STATIC_DAY_CACHE: &str = "public, max-age=86400";
/// Set on a stream when `stream::cap` lowered its fps or timeout.
pub const CAPPED_HEADER: &str = "x-shout-capped";

/// The parts of an incoming request that routing looks at.
#[derive(Debug, Clone, Copy, Default)]
pub struct Request<'a> {
    pub method: &'a str,
    /// Path as it appears in the URL, still percent-encoded.
    pub path: &'a str,
    pub query: Option<&'a str>,
    pub accept: Option<&'a str>,
    pub user_agent: Option<&'a str>,
}

#[derive(Debug)]
pub enum Body {
    Empty,
    Text(String),
    /// Serve this path from the ASSETS binding (web/dist). The reply's
    /// headers replace whatever the asset server sent.
    Asset(String),
    Stream(Box<Animation>),
}

#[derive(Debug)]
pub struct Reply {
    pub status: u16,
    pub headers: Vec<(&'static str, Cow<'static, str>)>,
    pub body: Body,
    pub event: Event,
}

impl Reply {
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(name))
            .map(|(_, v)| v.as_ref())
    }
}

pub fn handle(req: &Request) -> Reply {
    let path = req.path;
    // (route pattern, path parameter or "")
    let named = match path {
        "/" => Some(("/", "")),
        "/health" => Some(("/health", "")),
        "/favicon.ico" => Some(("/favicon.ico", "")),
        "/favicon.svg" => Some(("/favicon.svg", "")),
        "/og.png" => Some(("/og.png", "")),
        "/fonts" => Some(("/fonts", "")),
        "/presets" => Some(("/presets", "")),
        _ => [
            ("/fonts/", "/fonts/{name}"),
            ("/presets/", "/presets/{name}"),
            ("/_app/", "/_app/{file}"),
        ]
        .into_iter()
        .find_map(|(prefix, route)| Some((route, param(path, prefix)?))),
    };
    let Some((route, arg)) = named else {
        let mut reply = render_fallback(req);
        // A HEAD response has no body, but the runtime would still wait
        // for the whole stream before sending the headers.
        if req.method == "HEAD" && matches!(reply.body, Body::Stream(_)) {
            reply.body = Body::Empty;
        }
        return reply;
    };
    if req.method != "GET" && req.method != "HEAD" {
        return Reply {
            status: 405,
            headers: vec![("allow", "GET,HEAD".into())],
            body: Body::Empty,
            event: Event::route(route, 405),
        };
    }
    match route {
        "/" if accepts_html(req) => html_page("/", "/index.html"),
        "/" => plain("/", HELP.clone()),
        "/health" => plain("/health", "ok\n".into()),
        "/favicon.ico" => Reply {
            status: 204,
            headers: vec![],
            body: Body::Empty,
            event: Event::route(route, 204),
        },
        "/favicon.svg" => static_asset(route, "image/svg+xml"),
        "/og.png" => static_asset(route, "image/png"),
        "/fonts" => plain(route, FONTS_BODY.clone()),
        "/presets" => plain(route, PRESETS_BODY.clone()),
        "/fonts/{name}" => font_preview(&decode(arg)),
        "/presets/{name}" => preset_preview(&decode(arg)),
        _ => app_asset(&decode(arg)),
    }
}

/// Match a single non-empty path segment after `prefix`, like axum's
/// `/prefix/{name}`.
fn param<'a>(path: &'a str, prefix: &str) -> Option<&'a str> {
    let rest = path.strip_prefix(prefix)?;
    (!rest.is_empty() && !rest.contains('/')).then_some(rest)
}

fn decode(s: &str) -> String {
    percent_decode_str(s).decode_utf8_lossy().into_owned()
}

fn plain(route: &'static str, body: String) -> Reply {
    Reply {
        status: 200,
        headers: vec![("content-type", TEXT_PLAIN.into())],
        body: Body::Text(body),
        event: Event::route(route, 200),
    }
}

fn html_page(route: &'static str, asset: &str) -> Reply {
    Reply {
        status: 200,
        headers: vec![
            ("content-type", TEXT_HTML.into()),
            ("cache-control", HTML_CACHE.into()),
        ],
        body: Body::Asset(asset.into()),
        event: Event::route(route, 200),
    }
}

fn static_asset(route: &'static str, ctype: &'static str) -> Reply {
    Reply {
        status: 200,
        headers: vec![
            ("content-type", ctype.into()),
            ("cache-control", STATIC_DAY_CACHE.into()),
        ],
        body: Body::Asset(route.into()),
        event: Event::route(route, 200),
    }
}

fn not_found(route: &'static str) -> Reply {
    Reply {
        status: 404,
        headers: vec![],
        body: Body::Empty,
        event: Event::route(route, 404),
    }
}

fn app_asset(file: &str) -> Reply {
    const ROUTE: &str = "/_app/{file}";
    // Hashed esbuild names only use these characters. Anything else could
    // change meaning once the name goes back into a URL for the ASSETS
    // binding: a decoded `?` or `#` cuts the path short, `../` leaves _app.
    let safe = |b: u8| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-');
    if file.len() > MAX_PARAM_LEN || !file.bytes().all(safe) {
        return not_found(ROUTE);
    }
    // Only the types esbuild emits are served. Anything else in dist/_app
    // stays private, as it was when the server embedded a fixed list.
    let ctype = match file.rsplit_once('.').map(|(_, ext)| ext) {
        Some("js") => "text/javascript; charset=utf-8",
        Some("css") => "text/css; charset=utf-8",
        Some("wasm") => "application/wasm",
        _ => return not_found(ROUTE),
    };
    Reply {
        status: 200,
        headers: vec![
            ("content-type", ctype.into()),
            ("cache-control", IMMUTABLE_CACHE.into()),
        ],
        body: Body::Asset(format!("/_app/{file}")),
        event: Event::route(ROUTE, 200),
    }
}

fn error_response(
    route: &'static str,
    kind: RenderKind,
    cfg: &RenderConfig,
    err: RenderError,
) -> Reply {
    let status = match err {
        RenderError::EmptyText => 200,
        _ => 400,
    };
    Reply {
        status,
        headers: vec![("content-type", TEXT_PLAIN.into())],
        body: Body::Text(format!("{}\n", err.message())),
        event: Event::render(route, status, kind, cfg).with_error(&err),
    }
}

fn font_preview(name: &str) -> Reply {
    const ROUTE: &str = "/fonts/{name}";
    let lower = name.to_lowercase();
    // Preview with `sunset` so multi-color fonts show off their layers.
    let cfg = RenderConfig {
        text: "Hello World".into(),
        font: lower,
        preset: "sunset".into(),
        ..Default::default()
    };
    if name.len() > MAX_PARAM_LEN || !fonts::is_font(&cfg.font) {
        return error_response(ROUTE, RenderKind::Static, &cfg, RenderError::UnknownFont);
    }
    render_static(ROUTE, &cfg)
}

fn preset_preview(name: &str) -> Reply {
    const ROUTE: &str = "/presets/{name}";
    let cfg = RenderConfig {
        text: "Hello World".into(),
        preset: name.to_lowercase(),
        ..Default::default()
    };
    if name.len() > MAX_PARAM_LEN {
        return error_response(ROUTE, RenderKind::Static, &cfg, RenderError::UnknownPreset);
    }
    render_static(ROUTE, &cfg)
}

fn accepts_html(req: &Request) -> bool {
    req.accept.is_some_and(|s| s.contains("text/html"))
}

/// Browsers hitting a stream URL would hang a tab forever. Detect them by
/// Accept header or User-Agent prefix and force `once` → static frame.
fn is_browser(req: &Request) -> bool {
    accepts_html(req) || req.user_agent.is_some_and(|s| s.starts_with("Mozilla/"))
}

/// Paths that serve an HTML page to browsers but shout their name at curl.
fn browser_page(path: &str) -> Option<(&'static str, &'static str)> {
    match path {
        "/about" => Some(("/about", "/about.html")),
        "/privacy" => Some(("/privacy", "/privacy.html")),
        _ => None,
    }
}

fn render_fallback(req: &Request) -> Reply {
    let query_len = req.query.map(|q| q.len() + 1).unwrap_or(0);
    if req.path.len() + query_len > MAX_URL_LEN {
        return Reply {
            status: 414,
            headers: vec![("content-type", TEXT_PLAIN.into())],
            body: Body::Text("url too long.\n".into()),
            event: Event::route(ROUTE_RENDER, 414),
        };
    }
    let browser = is_browser(req);
    if browser && let Some((route, asset)) = browser_page(req.path) {
        return html_page(route, asset);
    }
    let mut cfg = parse(req.path, req.query);
    if browser {
        cfg.once = true;
    }

    // JSON always returns a single static frame.
    if !cfg.json && cfg.should_animate() {
        return match Animation::new(&cfg) {
            Ok(anim) => {
                let mut headers = vec![
                    ("content-type", TEXT_PLAIN.into()),
                    ("cache-control", "no-cache".into()),
                    ("x-content-type-options", "nosniff".into()),
                ];
                // A big banner streams slower or for less time; say so
                // here, since text in the body would corrupt the frames.
                if let Some((fps, timeout)) = anim.capped() {
                    headers.push((
                        CAPPED_HEADER,
                        format!("fps={fps}; timeout={timeout}").into(),
                    ));
                }
                Reply {
                    status: 200,
                    headers,
                    body: Body::Stream(Box::new(anim)),
                    event: Event::render(ROUTE_RENDER, 200, RenderKind::Animated, &cfg),
                }
            }
            Err(e) => error_response(ROUTE_RENDER, RenderKind::Animated, &cfg, e),
        };
    }
    render_static(ROUTE_RENDER, &cfg)
}

fn render_static(route: &'static str, cfg: &RenderConfig) -> Reply {
    let kind = if cfg.json {
        RenderKind::Json
    } else {
        RenderKind::Static
    };
    match render_config(cfg) {
        Ok(out) if cfg.json => {
            let body = serde_json::json!({
                "text": cfg.text,
                "font": cfg.font,
                "render": out,
            });
            Reply {
                status: 200,
                headers: vec![("content-type", "application/json".into())],
                body: Body::Text(body.to_string()),
                event: Event::render(route, 200, kind, cfg),
            }
        }
        Ok(out) => Reply {
            status: 200,
            headers: vec![("content-type", TEXT_PLAIN.into())],
            body: Body::Text(out),
            event: Event::render(route, 200, kind, cfg),
        },
        Err(e) => error_response(route, kind, cfg, e),
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
    s.push_str("  rainbow   animated hsl hue ring.\n");
    s.push_str("  fire      animated red/orange/yellow flicker.\n\n");
    s.push_str("ANIMATION\n");
    s.push_str("  animate      force animation on any mode.\n");
    s.push_str("  once         force a single static frame.\n");
    s.push_str("  ?fps=N       frames per second. default 10, capped at 30.\n");
    s.push_str("  ?timeout=N   seconds before server closes. default 60, max 300.\n");
    s.push_str("  large banners stream at lower fps or timeout; the X-Shout-Capped\n");
    s.push_str("  response header shows the values used.\n\n");
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
