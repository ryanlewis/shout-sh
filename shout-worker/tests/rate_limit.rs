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

//! Which requests are rate limited, under which limit, and the 429 reply.
//! The binding call itself only runs in the Worker; `just smoke` covers it.

use shout_worker::app::{
    Body, Limit, RETRY_AFTER, Request, handle, plan, rate_limit_key, too_many_requests,
};

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

fn rate_limit(r: &Request) -> Option<(Limit, &'static str)> {
    plan(r).limit()
}

fn limit(uri: &str) -> Option<Limit> {
    rate_limit(&req(uri)).map(|(limit, _)| limit)
}

#[test]
fn health_and_page_assets_are_exempt() {
    for uri in [
        "/health",
        "/favicon.ico",
        "/favicon.svg",
        "/og.png",
        "/_app/index-ABC123.js",
        "/_app/shout_wasm_bg-ABC123.wasm",
    ] {
        assert_eq!(limit(uri), None, "{uri}");
    }
}

/// Every method but GET and HEAD gets a 405 without rendering, but still
/// costs a Worker call, so it counts against the general limit. Exempt
/// routes and streams too. The route is the one the 405 records.
#[test]
fn other_methods_use_the_general_limit() {
    for (uri, route) in [
        ("/health", "/health"),
        ("/_app/index-ABC123.js", "/_app/{file}"),
        ("/fonts/block", "/fonts/{name}"),
        ("/about", "/about"),
        ("/hello", "/render"),
        ("/fire/boom", "/render"),
    ] {
        for method in ["POST", "PUT", "DELETE", "OPTIONS", "PROPFIND"] {
            let r = Request { method, ..req(uri) };
            let p = plan(&r);
            assert_eq!(p.limit(), Some((Limit::General, route)), "{method} {uri}");
            assert_eq!(p.stream_timeout_ms(), None, "{method} {uri}");
            assert_eq!(p.reply().status, 405, "{method} {uri}");
        }
    }
}

#[test]
fn render_and_listing_routes_use_the_general_limit() {
    for (uri, route) in [
        ("/", "/"),
        ("/fonts", "/fonts"),
        ("/fonts/block", "/fonts/{name}"),
        ("/presets", "/presets"),
        ("/presets/sunset", "/presets/{name}"),
        ("/hello", "/render"),
        ("/tiny/hello", "/render"),
        ("/hello?format=json", "/render"),
        ("/fire+once/boom", "/render"),
        ("/about", "/render"),
        // Not a single segment after /_app/, so it is a banner render.
        ("/_app/a/b", "/render"),
        ("/favicon.png", "/render"),
    ] {
        assert_eq!(
            rate_limit(&req(uri)),
            Some((Limit::General, route)),
            "{uri}"
        );
    }
}

#[test]
fn animations_use_the_stream_limit() {
    for uri in [
        "/fire/boom",
        "/rainbow/hi",
        "/animate/hi",
        "/hi?mode=fire",
        "/fire/boom?timeout=300",
    ] {
        assert_eq!(
            rate_limit(&req(uri)),
            Some((Limit::Stream, "/render")),
            "{uri}"
        );
    }
}

#[test]
fn head_and_json_never_use_the_stream_limit() {
    let head = Request {
        method: "HEAD",
        ..req("/fire/boom")
    };
    assert_eq!(rate_limit(&head), Some((Limit::General, "/render")));
    assert_eq!(limit("/fire/boom?format=json"), Some(Limit::General));
}

#[test]
fn browsers_use_the_general_limit() {
    let r = Request {
        user_agent: Some("Mozilla/5.0"),
        ..req("/fire/boom")
    };
    assert_eq!(rate_limit(&r), Some((Limit::General, "/render")));

    let r = Request {
        accept: Some("text/html"),
        ..req("/privacy")
    };
    assert_eq!(rate_limit(&r), Some((Limit::General, "/privacy")));
}

#[test]
fn oversize_url_uses_the_general_limit() {
    let long = format!("/fire/{}", "a".repeat(5000));
    assert_eq!(limit(&long), Some(Limit::General));
}

/// `rate_limit` guesses whether `handle` will stream without rendering.
/// The guess must agree with what `handle` does.
#[test]
fn stream_limit_matches_what_handle_streams() {
    let uris = [
        "/hello",
        "/fire/boom",
        "/fire+once/boom",
        "/rainbow/hi",
        "/animate/tiny/hi",
        "/solid/red/hi",
        "/hi?mode=rainbow",
        "/hi?mode=rainbow&format=json",
        "/fire/boom?fps=30&timeout=300",
        "/fire/boom?font=nope",
        "/fonts/block",
        "/presets/sunset",
    ];
    for uri in uris {
        for method in ["GET", "HEAD"] {
            for user_agent in [None, Some("Mozilla/5.0"), Some("curl/8.7.1")] {
                let r = Request {
                    method,
                    user_agent,
                    ..req(uri)
                };
                let streams = matches!(handle(&r).body, Body::Stream(_));
                let stream_limit = matches!(rate_limit(&r), Some((Limit::Stream, _)));
                // The glue checks a stream slot when there is a timeout.
                let slot = plan(&r).stream_timeout_ms().is_some();
                assert_eq!(slot, stream_limit, "{method} {uri} {user_agent:?}");
                // An animation that fails validation (unknown font) is a
                // 400, but it still counts as a stream request.
                if streams {
                    assert!(stream_limit, "{method} {uri} {user_agent:?}");
                } else if stream_limit {
                    assert_eq!(handle(&r).status, 400, "{method} {uri} {user_agent:?}");
                }
            }
        }
    }
}

#[test]
fn too_many_requests_is_plain_text_429() {
    let r = too_many_requests(Limit::General, "/render", RETRY_AFTER);
    assert_eq!(r.status, 429);
    assert_eq!(r.header("content-type"), Some("text/plain; charset=utf-8"));
    assert_eq!(r.header("retry-after"), Some("60"));
    assert_eq!(RETRY_AFTER, 60);
    match &r.body {
        Body::Text(s) => assert_eq!(s, "too many requests. try again in a minute.\n"),
        other => panic!("expected a text body, got {other:?}"),
    }
    assert_eq!(r.event.route, "/render");
    assert_eq!(r.event.status, 429);
    assert_eq!(r.event.kind, None);
    assert_eq!(
        r.event.blobs(),
        ["/render", "", "", "", "", "rate_limit_general"]
    );
}

#[test]
fn each_limit_records_its_own_reason() {
    let general = too_many_requests(Limit::General, "/fonts", RETRY_AFTER);
    assert_eq!(general.event.error, "rate_limit_general");
    let stream = too_many_requests(Limit::Stream, "/render", RETRY_AFTER);
    assert_eq!(stream.event.error, "rate_limit_stream");
    assert_eq!(
        stream.event.blobs(),
        ["/render", "", "", "", "", "rate_limit_stream"]
    );
}

#[test]
fn ipv4_keys_on_the_address() {
    assert_eq!(rate_limit_key("203.0.113.7"), "203.0.113.7");
}

#[test]
fn ipv6_keys_on_the_slash_64() {
    assert_eq!(
        rate_limit_key("2001:db8:1:2:aaaa:bbbb:cccc:dddd"),
        "2001:db8:1:2::/64"
    );
    assert_eq!(rate_limit_key("2001:db8:1:2::1"), "2001:db8:1:2::/64");
    assert_eq!(rate_limit_key("2001:db8::1"), "2001:db8:0:0::/64");
}

#[test]
fn unparseable_ip_is_used_as_is() {
    assert_eq!(rate_limit_key("not-an-ip"), "not-an-ip");
}

#[test]
fn ipv4_mapped_ipv6_keys_on_the_address() {
    assert_eq!(rate_limit_key("::ffff:203.0.113.7"), "::ffff:203.0.113.7");
}
