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

//! Routing and response assertions, run against `app::handle` on the
//! host. Asset bytes come from the ASSETS binding at runtime, so these
//! tests check which asset path is requested and with which headers;
//! `just smoke` checks the bytes through `wrangler dev`.

use shout_worker::app::{Body, Reply, Request, handle, help_text};
use shout_worker::event::RenderKind;

fn req(uri: &str) -> Request<'_> {
    let (path, query) = match uri.split_once('?') {
        Some((p, q)) => (p, Some(q)),
        None => (uri, None),
    };
    Request {
        method: "GET",
        path,
        query,
        accept: None,
        user_agent: None,
    }
}

fn get(uri: &str) -> Reply {
    handle(&req(uri))
}

fn get_with_accept(uri: &str, accept: &str) -> Reply {
    handle(&Request {
        accept: Some(accept),
        ..req(uri)
    })
}

fn ctype(r: &Reply) -> &'static str {
    r.header("content-type").unwrap_or_default()
}

fn text(r: &Reply) -> &str {
    match &r.body {
        Body::Text(s) => s,
        other => panic!("expected a text body, got {other:?}"),
    }
}

fn asset(r: &Reply) -> &str {
    match &r.body {
        Body::Asset(p) => p,
        other => panic!("expected an asset body, got {other:?}"),
    }
}

#[test]
fn health_ok() {
    let r = get("/health");
    assert_eq!(r.status, 200);
    assert!(ctype(&r).starts_with("text/plain"));
    assert_eq!(text(&r), "ok\n");
}

#[test]
fn favicon_is_no_content() {
    let r = get("/favicon.ico");
    assert_eq!(r.status, 204);
    assert!(matches!(r.body, Body::Empty));
}

#[test]
fn favicon_svg_and_og_png_come_from_assets() {
    let r = get("/favicon.svg");
    assert_eq!(r.status, 200);
    assert_eq!(asset(&r), "/favicon.svg");
    assert_eq!(ctype(&r), "image/svg+xml");
    assert_eq!(r.header("cache-control"), Some("public, max-age=86400"));

    let r = get("/og.png");
    assert_eq!(asset(&r), "/og.png");
    assert_eq!(ctype(&r), "image/png");
    assert_eq!(r.header("cache-control"), Some("public, max-age=86400"));
}

#[test]
fn root_help_text_includes_usage() {
    let r = get("/");
    assert_eq!(r.status, 200);
    assert!(ctype(&r).starts_with("text/plain"));
    let body = text(&r);
    assert!(body.contains("USAGE"));
    assert!(body.contains("FONTS"));
    assert!(body.contains("MODES"));
    assert_eq!(body, help_text());
}

#[test]
fn fonts_lists_thirteen() {
    let r = get("/fonts");
    assert_eq!(r.status, 200);
    let lines: Vec<_> = text(&r).lines().filter(|l| !l.is_empty()).collect();
    assert_eq!(lines.len(), 13);
    assert!(lines.contains(&"block"));
    assert!(lines.contains(&"tiny"));
}

#[test]
fn font_preview_renders() {
    let r = get("/fonts/block");
    assert_eq!(r.status, 200);
    assert!(!text(&r).is_empty());
}

#[test]
fn font_preview_name_is_percent_decoded() {
    let r = get("/fonts/bl%6Fck");
    assert_eq!(r.status, 200);
}

#[test]
fn font_preview_unknown_400() {
    let r = get("/fonts/standard");
    assert_eq!(r.status, 400);
    assert!(text(&r).contains("font not found"));
}

#[test]
fn font_preview_over_param_cap_400() {
    let r = get(&format!("/fonts/{}", "a".repeat(65)));
    assert_eq!(r.status, 400);
    assert!(text(&r).contains("font not found"));
}

#[test]
fn presets_lists_all() {
    let r = get("/presets");
    assert_eq!(r.status, 200);
    let lines: Vec<_> = text(&r).lines().filter(|l| !l.is_empty()).collect();
    assert!(lines.contains(&"sunset"));
    assert!(lines.contains(&"ocean"));
}

#[test]
fn preset_preview_renders() {
    let r = get("/presets/sunset");
    assert_eq!(r.status, 200);
    assert!(text(&r).contains("\x1b[38;2;"));
}

#[test]
fn preset_preview_unknown_400() {
    let r = get("/presets/puce");
    assert_eq!(r.status, 400);
    assert!(text(&r).contains("preset not found"));
}

#[test]
fn preset_directive_renders_truecolor() {
    let r = get("/sunset/hi");
    assert_eq!(r.status, 200);
    assert!(text(&r).contains("\x1b[38;2;"));
}

#[test]
fn single_segment_renders_text() {
    let r = get("/HELLO");
    assert_eq!(r.status, 200);
    assert!(!text(&r).is_empty());
}

#[test]
fn red_tiny_hello_emits_red_sgr() {
    let r = get("/tiny+red/hello+world");
    assert_eq!(r.status, 200);
    assert!(text(&r).contains("\x1b[31m"), "expected red SGR");
}

#[test]
fn fire_once_emits_truecolor_sgr() {
    // Without `once`, /fire/ALERT streams.
    let r = get("/fire+once/ALERT");
    assert_eq!(r.status, 200);
    assert!(text(&r).contains("\x1b[38;2;"), "expected truecolor SGR");
}

#[test]
fn json_format_returns_json() {
    let r = get("/HELLO?format=json");
    assert_eq!(r.status, 200);
    assert!(ctype(&r).starts_with("application/json"));
    let v: serde_json::Value = serde_json::from_str(text(&r)).unwrap();
    assert_eq!(v["text"], "HELLO");
    assert_eq!(v["font"], "block");
    assert!(!v["render"].as_str().unwrap().is_empty());
}

#[test]
fn rainbow_once_end_to_end() {
    // rainbow animates by default; `once` forces a static frame.
    let r = get("/rainbow+once/party");
    assert_eq!(r.status, 200);
    assert!(
        text(&r).contains("\x1b[38;2;"),
        "expected truecolor SGR in rainbow output"
    );
}

#[test]
fn solid_without_color_is_white() {
    let r = get("/solid/hi");
    assert_eq!(r.status, 200);
    assert!(text(&r).contains("\x1b["), "expected SGR in solid output");
}

#[test]
fn query_font_overrides_path_font_over_http() {
    let r = get("/tiny/hi?font=block");
    assert_eq!(r.status, 200);
    // block font uses ╗ box chars; tiny does not. Asserts the override took.
    assert!(
        text(&r).contains('╗'),
        "expected block-font glyphs after override"
    );
}

#[test]
fn query_mode_overrides_path_mode_over_http() {
    // path says solid (no SGR truecolor), query flips to fire (truecolor).
    let r = get("/solid+once/hi?mode=fire");
    assert_eq!(r.status, 200);
    assert!(
        text(&r).contains("\x1b[38;2;"),
        "expected fire truecolor after override"
    );
}

#[test]
fn layout_directive_ignored_not_400() {
    // `full` is a legacy FIGlet layout — accept silently.
    let r = get("/full/Hi");
    assert_eq!(r.status, 200);
    assert!(!text(&r).is_empty());
}

#[test]
fn width_directive_ignored_not_400() {
    let r = get("/w120/Hi");
    assert_eq!(r.status, 200);
    assert!(!text(&r).is_empty());
}

#[test]
fn animate_once_flags_ignored_not_400() {
    let r = get("/rainbow+once/Hi");
    assert_eq!(r.status, 200);
    assert!(!text(&r).is_empty());
}

#[test]
fn unknown_mode_via_query_is_silently_ignored() {
    let r = get("/Hi?mode=matrix");
    assert_eq!(r.status, 200);
    assert!(ctype(&r).starts_with("text/plain"));
}

#[test]
fn unknown_font_via_query_is_400() {
    let r = get("/Hi?font=standard");
    assert_eq!(r.status, 400);
    assert!(text(&r).contains("font not found"));
}

#[test]
fn unknown_color_via_query_is_400() {
    let r = get("/Hi?color=puce");
    assert_eq!(r.status, 400);
    assert!(text(&r).contains("color not found"));
}

#[test]
fn very_long_text_within_cap_renders_ok() {
    let long = "a".repeat(200);
    let r = get(&format!("/{long}"));
    assert_eq!(r.status, 200);
    assert!(!text(&r).is_empty());
}

#[test]
fn oversize_url_rejected_with_414() {
    let huge = "a".repeat(1000);
    let r = get(&format!("/{huge}"));
    assert_eq!(r.status, 414);
    assert_eq!(text(&r), "url too long.\n");
}

#[test]
fn oversize_query_counts_towards_414() {
    let r = get(&format!("/hi?x={}", "a".repeat(600)));
    assert_eq!(r.status, 414);
}

#[test]
fn escape_sequences_in_text_are_stripped() {
    // %1B is literal ESC. If it leaked into the banner body a caller could
    // smuggle terminal-hijack sequences through shout.sh.
    let r = get("/%1B%5B31mPWNED");
    assert_eq!(r.status, 200);
    assert!(!text(&r).contains("\x1b[31mPWNED"));
}

#[test]
fn unicode_text_renders_or_degrades_gracefully() {
    // Workers hands us the percent-encoded path, as curl sends it.
    let r = get("/caf%C3%A9");
    assert_eq!(r.status, 200);
}

#[test]
fn empty_directive_segment_is_text() {
    let r = get("//Hi");
    assert_eq!(r.status, 200);
}

#[test]
fn double_plus_does_not_panic() {
    let r = get("/hello++world");
    assert_eq!(r.status, 200);
}

#[test]
fn trailing_slash_empty_text_friendly() {
    let r = get("/block/");
    assert_eq!(r.status, 200);
    assert!(text(&r).contains("nothing to shout about"));
}

#[test]
fn valueless_query_param_is_ignored() {
    let r = get("/HELLO?format");
    assert_eq!(r.status, 200);
    assert!(
        ctype(&r).starts_with("text/plain"),
        "format should stay unset"
    );
}

#[test]
fn empty_query_value_is_ignored() {
    let r = get("/HELLO?font=&format=json");
    assert_eq!(r.status, 200);
    assert!(ctype(&r).starts_with("application/json"));
}

#[test]
fn unknown_path_directives_fall_through_to_text() {
    let r = get("/notafont/Hi");
    assert_eq!(r.status, 200);
}

#[test]
fn root_html_accept_returns_playground() {
    let r = get_with_accept("/", "text/html");
    assert_eq!(r.status, 200);
    assert!(ctype(&r).starts_with("text/html"));
    assert_eq!(asset(&r), "/index.html");
}

#[test]
fn root_plain_accept_returns_help() {
    let r = get_with_accept("/", "text/plain");
    assert_eq!(r.status, 200);
    assert!(ctype(&r).starts_with("text/plain"));
    assert!(text(&r).contains("USAGE"));
}

#[test]
fn root_ignores_browser_user_agent() {
    // `/` only switches on Accept, unlike the stream paths.
    let r = handle(&Request {
        user_agent: Some("Mozilla/5.0"),
        ..req("/")
    });
    assert!(text(&r).contains("USAGE"));
}

#[test]
fn about_and_privacy_are_html_for_browsers() {
    for (path, file) in [("/about", "/about.html"), ("/privacy", "/privacy.html")] {
        let r = get_with_accept(path, "text/html");
        assert_eq!(asset(&r), file);
        assert!(ctype(&r).starts_with("text/html"));
        let r = handle(&Request {
            user_agent: Some("Mozilla/5.0"),
            ..req(path)
        });
        assert_eq!(asset(&r), file);
    }
}

#[test]
fn about_and_privacy_are_banners_for_curl() {
    for path in ["/about", "/privacy"] {
        let r = handle(&Request {
            user_agent: Some("curl/8.7.1"),
            accept: Some("*/*"),
            ..req(path)
        });
        assert_eq!(r.status, 200);
        assert!(ctype(&r).starts_with("text/plain"));
        assert!(!text(&r).is_empty());
    }
}

#[test]
fn app_asset_types_by_extension() {
    for (file, want) in [
        ("main-ABC123.js", "text/javascript; charset=utf-8"),
        ("styles-ABC123.css", "text/css; charset=utf-8"),
        ("shout_wasm_bg-0123abcd.wasm", "application/wasm"),
    ] {
        let r = get(&format!("/_app/{file}"));
        assert_eq!(r.status, 200, "{file}");
        assert_eq!(ctype(&r), want, "{file}");
        assert_eq!(asset(&r), format!("/_app/{file}"));
    }
}

#[test]
fn app_asset_unknown_extension_is_404() {
    let r = get("/_app/manifest.txt");
    assert_eq!(r.status, 404);
    assert!(matches!(r.body, Body::Empty));
}

#[test]
fn app_asset_over_param_cap_is_404() {
    let r = get(&format!("/_app/{}.js", "a".repeat(64)));
    assert_eq!(r.status, 404);
}

#[test]
fn app_asset_rejects_names_that_change_the_asset_url() {
    // Decoded, these would become `?`, `#` or `../` in the ASSETS URL.
    for file in [
        "main-ABC123.js%3F.css",
        "x%23.js",
        "..%2Fsecret.js",
        "a%20b.js",
    ] {
        let r = get(&format!("/_app/{file}"));
        assert_eq!(r.status, 404, "{file}");
        assert!(matches!(r.body, Body::Empty), "{file}");
    }
}

#[test]
fn app_asset_nested_path_falls_through_to_render() {
    // `/_app/{file}` is one segment, as it was in axum.
    let r = get("/_app/a/b.js");
    assert!(matches!(r.body, Body::Text(_)));
    assert_eq!(r.event.route, "/render");
}

#[test]
fn app_asset_is_immutable_cacheable() {
    let r = get("/_app/main-ABC123.js");
    let cc = r.header("cache-control").unwrap_or_default();
    assert!(cc.contains("immutable"), "got: {cc}");
    assert!(cc.contains("max-age="), "got: {cc}");
}

#[test]
fn html_pages_are_no_cache() {
    for path in ["/", "/about", "/privacy"] {
        let r = get_with_accept(path, "text/html");
        let cc = r.header("cache-control").unwrap_or_default();
        assert!(cc.contains("no-cache"), "{path}: got {cc:?}");
    }
}

#[test]
fn post_on_named_route_is_405() {
    let r = handle(&Request {
        method: "POST",
        ..req("/health")
    });
    assert_eq!(r.status, 405);
    assert_eq!(r.header("allow"), Some("GET,HEAD"));
}

#[test]
fn head_on_named_route_is_ok() {
    let r = handle(&Request {
        method: "HEAD",
        ..req("/health")
    });
    assert_eq!(r.status, 200);
}

#[test]
fn events_use_route_patterns_not_raw_paths() {
    assert_eq!(get("/fonts/block").event.route, "/fonts/{name}");
    assert_eq!(get("/presets/ocean").event.route, "/presets/{name}");
    assert_eq!(get("/_app/x.js").event.route, "/_app/{file}");
    assert_eq!(get("/SECRET+TEXT").event.route, "/render");
    assert_eq!(get("/health").event.route, "/health");
}

#[test]
fn events_never_carry_user_text() {
    for uri in [
        "/tiny/SECRET",
        "/SECRET?font=SECRET",
        "/SECRET?format=json",
        "/rainbow/SECRET",
        "/fonts/SECRET",
        "/presets/SECRET",
    ] {
        let e = get(uri).event;
        for blob in e.blobs() {
            assert!(!blob.to_lowercase().contains("secret"), "{uri}: {blob}");
        }
    }
}

#[test]
fn events_record_kind_font_mode_status() {
    let e = get("/tiny/hi").event;
    assert_eq!(e.kind, Some(RenderKind::Static));
    assert_eq!(e.font, "tiny");
    assert_eq!(e.mode, "default");
    assert_eq!(e.status, 200);

    let e = get("/hi?format=json").event;
    assert_eq!(e.kind, Some(RenderKind::Json));

    let e = get("/fire/hi").event;
    assert_eq!(e.kind, Some(RenderKind::Animated));
    assert_eq!(e.mode, "fire");

    let e = get("/hi?font=standard").event;
    assert_eq!(e.status, 400);
    assert_eq!(e.font, "");

    let e = get("/health").event;
    assert_eq!(e.kind, None);
    assert_eq!(e.blobs(), ["/health", "", "", ""]);
    assert_eq!(e.doubles(7, 0), [200.0, 7.0, 0.0]);
}
