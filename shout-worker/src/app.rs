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
use std::net::Ipv6Addr;
use std::sync::LazyLock;

use percent_encoding::percent_decode_str;

use shout_core::fonts;
use shout_core::parser::{MAX_PARAM_LEN, MAX_URL_LEN, RenderConfig, parse};
use shout_core::presets;
use shout_core::render::{RenderError, banner, render_config};

use crate::event::{Event, ROUTE_RENDER, RenderKind};
use crate::slots;
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

/// The reply to `req` when no limit refuses it. The Worker calls `plan`
/// and `Plan::reply` itself, with the limit checks in between.
pub fn handle(req: &Request) -> Reply {
    plan(req).reply()
}

/// What happens to a request, decided before anything is rendered: the
/// method check, the rate limit, and whether the reply is a stream.
#[derive(Debug)]
pub struct Plan<'a> {
    req: Request<'a>,
    target: Target<'a>,
}

/// Where a request goes. Parsed once, by `plan`; `Plan::reply` renders
/// from it, so the limit and the reply cannot disagree.
#[derive(Debug)]
enum Target<'a> {
    /// Any method but GET and HEAD: every route only reads. Holds the
    /// route the 405 records.
    NotAllowed(&'static str),
    /// A named route, as (route pattern, path parameter or "").
    Named(&'static str, &'a str),
    /// No named route matched; the banner renderer handles it.
    Fallback(Fallback),
}

/// Plan `req`. This is the only method check, and the only parse of a
/// banner request. It must not render anything: it runs before any
/// limit is checked.
pub fn plan<'a>(req: &Request<'a>) -> Plan<'a> {
    let target = if !matches!(req.method, "GET" | "HEAD") {
        Target::NotAllowed(method_route(req.path))
    } else if let Some((route, arg)) = named_route(req.path) {
        Target::Named(route, arg)
    } else {
        Target::Fallback(fallback(req))
    };
    Plan { req: *req, target }
}

impl Plan<'_> {
    /// Which rate limit applies, and the route to record if it refuses the
    /// request. `None` means the request is exempt: the health check, and
    /// the files a browser loads with every page, when fetched with GET or
    /// HEAD. Any other method counts against the general limit: it gets a
    /// 405 without rendering, but still costs a Worker call and an
    /// analytics write.
    pub fn limit(&self) -> Option<(Limit, &'static str)> {
        match &self.target {
            &Target::NotAllowed(route) => Some((Limit::General, route)),
            Target::Named(
                "/health" | "/favicon.ico" | "/favicon.svg" | "/og.png" | "/_app/{file}",
                _,
            ) => None,
            &Target::Named(route, _) => Some((Limit::General, route)),
            Target::Fallback(_) if self.stream().is_some() => Some((Limit::Stream, ROUTE_RENDER)),
            &Target::Fallback(Fallback::Page(route, _)) => Some((Limit::General, route)),
            Target::Fallback(_) => Some((Limit::General, ROUTE_RENDER)),
        }
    }

    /// For an animation stream, the timeout the request asked for, in ms.
    /// The stream slot's lease is sized from it. `stream::cap` can only
    /// lower the timeout, never raise it, so this is an upper bound, and
    /// the slot can be checked before `reply` renders frame 0.
    pub fn stream_timeout_ms(&self) -> Option<u64> {
        self.stream().map(|cfg| u64::from(cfg.timeout) * 1000)
    }

    /// The config of an animation stream, if the reply is one. HEAD never
    /// streams; see `reply`. An animation that fails validation (an
    /// unknown font, say) is still a stream here, and a 400 from `reply`.
    fn stream(&self) -> Option<&RenderConfig> {
        match &self.target {
            Target::Fallback(Fallback::Render(cfg))
                if self.req.method != "HEAD" && cfg.should_animate() =>
            {
                Some(cfg)
            }
            _ => None,
        }
    }

    /// Route the request and build its reply. This renders any banner.
    pub fn reply(&self) -> Reply {
        match &self.target {
            &Target::NotAllowed(route) => method_not_allowed(route),
            &Target::Named(route, arg) => serve(&self.req, route, arg),
            Target::Fallback(fb) => {
                let mut reply = render_fallback(fb);
                // A HEAD response has no body, but the runtime would still
                // wait for the whole stream before sending the headers.
                if self.req.method == "HEAD" && matches!(reply.body, Body::Stream(_)) {
                    reply.body = Body::Empty;
                }
                reply
            }
        }
    }
}

/// The reply for a named route.
fn serve(req: &Request, route: &'static str, arg: &str) -> Reply {
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

/// A per-client rate limit. Each one is a `[[ratelimits]]` binding in
/// wrangler.toml, which holds the numbers; see the comment there.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Limit {
    /// Everything that is not exempt and not an animation stream.
    General,
    /// Animation streams. Stricter, because each one holds a connection
    /// for up to 300s and sends up to `stream::MAX_STREAM_BYTES`.
    Stream,
    /// Too many animation streams open at once. Not a `[[ratelimits]]`
    /// binding and never returned by `Plan::limit`: the `StreamSlots`
    /// Durable Object applies it, see `slots` and
    /// `Plan::stream_timeout_ms`.
    StreamSlots,
}

impl Limit {
    /// The label recorded in blob6 when this limit refuses a request.
    /// A new limit adds an arm here.
    pub fn reason(self) -> &'static str {
        match self {
            Self::General => "rate_limit_general",
            Self::Stream => "rate_limit_stream",
            Self::StreamSlots => "stream_slots",
        }
    }
}

/// Seconds a refused client should wait. Both bindings count over a 60s
/// window (`period = 60` in wrangler.toml), so a minute always clears it.
pub const RETRY_AFTER: u64 = 60;

/// The reply for a client that `limit` refused, recorded under `route`.
/// `retry_after` is in seconds. Plain text for browsers too: an HTML page
/// would cost more to serve than the request it refuses.
pub fn too_many_requests(limit: Limit, route: &'static str, retry_after: u64) -> Reply {
    let body = match limit {
        Limit::StreamSlots => format!(
            "too many open streams. at most {} at a time; close one and try again.\n",
            slots::MAX_STREAMS
        ),
        Limit::General | Limit::Stream => "too many requests. try again in a minute.\n".into(),
    };
    Reply {
        status: 429,
        headers: vec![
            ("content-type", TEXT_PLAIN.into()),
            ("retry-after", retry_after.to_string().into()),
        ],
        body: Body::Text(body),
        event: Event {
            error: limit.reason(),
            ..Event::route(route, 429)
        },
    }
}

/// The route a 405 records: the named route or browser page `path`
/// matches, or `/render`.
fn method_route(path: &str) -> &'static str {
    named_route(path)
        .map(|(route, _)| route)
        .or_else(|| browser_page(path).map(|(route, _)| route))
        .unwrap_or(ROUTE_RENDER)
}

fn method_not_allowed(route: &'static str) -> Reply {
    Reply {
        status: 405,
        headers: vec![
            ("content-type", TEXT_PLAIN.into()),
            ("allow", "GET, HEAD".into()),
        ],
        body: Body::Text("method not allowed.\n".into()),
        event: Event::route(route, 405),
    }
}

/// The rate-limit key for a client IP. An IPv6 client usually holds a
/// whole /64, so it is keyed on that prefix; otherwise one client could
/// get a fresh limit per address. IPv4 addresses are keyed as they are.
pub fn rate_limit_key(ip: &str) -> String {
    match ip.parse::<Ipv6Addr>() {
        Ok(v6) if v6.to_ipv4_mapped().is_none() => {
            let s = v6.segments();
            format!("{:x}:{:x}:{:x}:{:x}::/64", s[0], s[1], s[2], s[3])
        }
        _ => ip.into(),
    }
}

/// The named route `path` matches, as (route pattern, path parameter or
/// ""). `None` means the banner renderer handles it.
fn named_route(path: &str) -> Option<(&'static str, &str)> {
    match path {
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

fn url_too_long(req: &Request) -> bool {
    let query_len = req.query.map(|q| q.len() + 1).unwrap_or(0);
    req.path.len() + query_len > MAX_URL_LEN
}

/// What the banner renderer does with a request no named route matched.
#[derive(Debug)]
enum Fallback {
    TooLong,
    /// An HTML page for a browser, as (route, asset).
    Page(&'static str, &'static str),
    /// Render a banner. A browser's config already has `once` set.
    Render(RenderConfig),
}

fn fallback(req: &Request) -> Fallback {
    if url_too_long(req) {
        return Fallback::TooLong;
    }
    let browser = is_browser(req);
    if browser && let Some((route, asset)) = browser_page(req.path) {
        return Fallback::Page(route, asset);
    }
    let mut cfg = parse(req.path, req.query);
    if browser {
        cfg.once = true;
    }
    Fallback::Render(cfg)
}

fn render_fallback(fb: &Fallback) -> Reply {
    let cfg = match fb {
        Fallback::TooLong => {
            return Reply {
                status: 414,
                headers: vec![("content-type", TEXT_PLAIN.into())],
                body: Body::Text("url too long.\n".into()),
                event: Event::route(ROUTE_RENDER, 414),
            };
        }
        &Fallback::Page(route, asset) => return html_page(route, asset),
        Fallback::Render(cfg) => cfg,
    };

    // JSON always returns a single static frame.
    if !cfg.json && cfg.should_animate() {
        return match Animation::new(cfg) {
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
                    event: Event::render(ROUTE_RENDER, 200, RenderKind::Animated, cfg),
                }
            }
            Err(e) => error_response(ROUTE_RENDER, RenderKind::Animated, cfg, e),
        };
    }
    render_static(ROUTE_RENDER, cfg)
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
