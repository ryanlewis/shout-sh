// shout.sh — curl-friendly ANSI banner service
// Copyright (C) 2026 Ryan Lewis
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

//! Stream framing and pacing, driven with a fake clock.

use shout_core::fonts::FONTS;
use shout_core::parser::{MAX_FPS, MAX_TIMEOUT};
use shout_worker::app::{Body, CAPPED_HEADER, Reply, Request, handle};
use shout_worker::stream::{Animation, MAX_BYTES_PER_SEC, MAX_STREAM_BYTES, Step, cap};

fn get_with(uri: &str, accept: Option<&str>, user_agent: Option<&str>) -> Reply {
    let (path, query) = match uri.split_once('?') {
        Some((p, q)) => (p, Some(q)),
        None => (uri, None),
    };
    handle(&Request {
        method: "GET",
        path,
        query,
        accept,
        user_agent,
    })
}

fn get(uri: &str) -> Reply {
    get_with(uri, None, None)
}

fn animation(r: Reply) -> Animation {
    match r.body {
        Body::Stream(a) => *a,
        other => panic!("expected a stream, got {other:?}"),
    }
}

fn text(r: &Reply) -> &str {
    match &r.body {
        Body::Text(s) => s,
        other => panic!("expected a text body, got {other:?}"),
    }
}

/// Run a stream to completion on a fake clock that jumps straight to each
/// requested wake-up. Returns every chunk sent.
fn run(mut anim: Animation) -> Vec<String> {
    let mut now = 1_000_000;
    let mut chunks = Vec::new();
    loop {
        match anim.step(now) {
            Step::Frame(s) | Step::End(s) => chunks.push(s),
            Step::Wait(ms) => {
                assert!(ms > 0, "Wait(0) would spin");
                now += ms;
            }
            Step::Done => return chunks,
        }
    }
}

#[test]
fn rainbow_streams_frames_with_cursor_controls() {
    let r = get("/rainbow/hi");
    assert_eq!(r.status, 200);
    assert!(r.header("content-type").unwrap().starts_with("text/plain"));
    assert_eq!(r.header("x-content-type-options"), Some("nosniff"));
    assert_eq!(r.header("cache-control"), Some("no-cache"));

    let mut anim = animation(r);
    let Step::Frame(c1) = anim.step(0) else {
        panic!("first step must send a frame")
    };
    assert_eq!(anim.step(0), Step::Wait(100), "10fps default");
    let Step::Frame(c2) = anim.step(100) else {
        panic!("frame due at 100ms")
    };

    assert!(
        c1.starts_with("\x1b[?25l\x1b[2J\x1b[H"),
        "hide, clear, home"
    );
    // second chunk begins with cursor-up-N then \r
    assert!(
        c2.starts_with("\x1b[") && c2.contains("A\r"),
        "second chunk should overwrite via cursor-up, got {:?}",
        &c2[..c2.len().min(20)]
    );
    assert!(c1.contains("\x1b[38;2;"));
    assert!(c2.contains("\x1b[38;2;"));
    assert_ne!(c1, c2, "rainbow animates");
}

#[test]
fn rainbow_once_returns_single_static_chunk() {
    let r = get("/rainbow+once/hi");
    assert_eq!(r.status, 200);
    let s = text(&r);
    assert!(
        !s.contains("\x1b[?25l"),
        "static response must not hide cursor"
    );
    assert!(!s.contains("A\r"), "static response must not use cursor-up");
    assert!(s.contains("\x1b[38;2;"), "expected truecolor SGR");
}

#[test]
fn fire_stream_completes_and_resets() {
    let chunks = run(animation(get("/fire/alert?fps=30&timeout=1")));
    let s = chunks.concat();
    assert!(
        s.starts_with("\x1b[?25l"),
        "stream must start with cursor hide"
    );
    assert!(
        s.ends_with("\x1b[0m\x1b[?25h\n"),
        "stream must end with SGR/cursor reset"
    );
}

#[test]
fn browser_accept_html_forces_static() {
    let r = get_with("/rainbow/hi", Some("text/html,application/xhtml+xml"), None);
    assert_eq!(r.status, 200);
    let s = text(&r);
    assert!(!s.contains("\x1b[?25l"), "browser must get static response");
    assert!(s.contains("\x1b[38;2;"), "expected truecolor SGR");
}

#[test]
fn browser_user_agent_forces_static() {
    let r = get_with("/rainbow/hi", None, Some("Mozilla/5.0 (Macintosh)"));
    assert!(!text(&r).contains("\x1b[?25l"));
}

#[test]
fn json_on_animated_mode_is_static_json() {
    let r = get("/rainbow/hi?format=json");
    assert_eq!(r.status, 200);
    assert!(
        r.header("content-type")
            .unwrap()
            .starts_with("application/json")
    );
    let v: serde_json::Value = serde_json::from_str(text(&r)).unwrap();
    assert_eq!(v["text"], "hi");
    assert!(v["render"].as_str().unwrap().contains("\x1b[38;2;"));
}

#[test]
fn fps_clamped_bounds_frame_count() {
    // ?fps=9999 clamps to 30 → 33ms ticks; ?timeout=1 → frames at 0, 33,
    // …, 990 (31 frames) plus the reset chunk.
    let chunks = run(animation(get("/rainbow/hi?fps=9999&timeout=1")));
    assert_eq!(chunks.len(), 32, "fps was not clamped");
}

#[test]
fn default_fps_and_timeout() {
    // 10fps for 60s: frames at 0, 100, …, 59_900 plus the reset chunk.
    let chunks = run(animation(get("/rainbow/hi")));
    assert_eq!(chunks.len(), 601);
}

#[test]
fn timeout_is_capped() {
    // ?timeout=9999 clamps to 300s; 1fps gives frames at 0..=299s.
    let chunks = run(animation(get("/rainbow/hi?fps=1&timeout=9999")));
    assert_eq!(chunks.len(), 301);
}

#[test]
fn timeout_zero_clamps_to_default_not_hang() {
    // ?timeout=0 → default 60s, not an instant close or a hang.
    let chunks = run(animation(get("/rainbow/hi?timeout=0&fps=1")));
    assert_eq!(chunks.len(), 61);
}

#[test]
fn late_wakeup_skips_missed_ticks() {
    let mut anim = animation(get("/rainbow/hi?fps=10&timeout=5"));
    assert!(matches!(anim.step(0), Step::Frame(_)));
    // Wake 350ms late: one catch-up frame, then wait for the 400ms tick.
    assert!(matches!(anim.step(450), Step::Frame(_)));
    assert_eq!(anim.step(450), Step::Wait(50));
}

#[test]
fn wait_never_overshoots_the_deadline() {
    let mut anim = animation(get("/rainbow/hi?fps=1&timeout=1"));
    assert!(matches!(anim.step(0), Step::Frame(_)));
    assert_eq!(anim.step(400), Step::Wait(600));
    assert!(matches!(anim.step(1000), Step::End(_)));
    assert_eq!(anim.step(1000), Step::Done);
}

#[test]
fn solid_never_animates() {
    let r = get("/solid/hi");
    assert!(!text(&r).contains("\x1b[?25l"));
}

#[test]
fn animate_forces_stream_on_solid() {
    let r = get("/solid+animate/ok");
    assert!(matches!(r.body, Body::Stream(_)));
}

#[test]
fn head_on_animated_path_does_not_stream() {
    let r = handle(&Request {
        method: "HEAD",
        path: "/rainbow/hi",
        ..Default::default()
    });
    assert_eq!(r.status, 200);
    assert_eq!(r.header("x-content-type-options"), Some("nosniff"));
    assert!(matches!(r.body, Body::Empty));
}

/// The heaviest stream the parser accepts: 200 wide letters in `3d` with
/// the largest letter spacing and padding. About 1.36 MB a frame.
fn worst_case_uri() -> String {
    format!(
        "/rainbow+3d/{}?ls=10&pad=10&fps=30&timeout=300",
        "W".repeat(200)
    )
}

#[test]
fn cap_leaves_small_frames_alone() {
    assert_eq!(cap(1_000, 10, 60), (10, 60));
    assert_eq!(cap(1_000, MAX_FPS, MAX_TIMEOUT), (MAX_FPS, MAX_TIMEOUT));
}

#[test]
fn cap_lowers_fps_then_timeout() {
    // 200 KB frames: 5 fps fits 1 MB/s; 64 MB at 1 MB/s is 64 s.
    assert_eq!(cap(200_000, 30, 300), (5, 64));
    // Bigger than a second's budget: 1 fps, and 64 MB / 2 MB is 32 s.
    assert_eq!(cap(2_000_000, 30, 300), (1, 32));
    // Bigger than the whole stream's budget: one frame a second for 1 s.
    assert_eq!(cap(100_000_000, 30, 300), (1, 1));
}

#[test]
fn cap_never_raises_fps_or_timeout() {
    for bytes in [
        0,
        1,
        999,
        33_334,
        100_000,
        1_000_001,
        5_000_000,
        u32::MAX as usize,
    ] {
        for fps in 1..=MAX_FPS {
            for timeout in [1, 2, 59, 60, 61, MAX_TIMEOUT] {
                let (f, t) = cap(bytes, fps, timeout);
                assert!(
                    f <= fps && t <= timeout,
                    "cap({bytes}, {fps}, {timeout}) = ({f}, {t})"
                );
                assert!(
                    f >= 1 && t >= 1,
                    "cap({bytes}, {fps}, {timeout}) = ({f}, {t})"
                );
            }
        }
    }
}

#[test]
fn normal_words_are_never_capped_at_defaults() {
    // "hello world again" in 3d is the largest of these, about 58 KB a
    // frame: 0.6 MB/s and 35 MB over 60 s.
    for word in ["hello", "shout.sh", "HELLO+WORLD", "hello+world+again"] {
        for mode in ["rainbow", "fire", "solid+animate"] {
            for font in FONTS {
                let uri = format!("/{mode}+{font}/{word}");
                let r = get(&uri);
                assert_eq!(r.header(CAPPED_HEADER), None, "{uri}");
                let mut anim = animation(r);
                assert_eq!(anim.capped(), None, "{uri}");
                assert!(matches!(anim.step(0), Step::Frame(_)));
                assert_eq!(anim.step(0), Step::Wait(100), "{uri}: 10 fps");
            }
        }
    }
}

#[test]
fn worst_case_is_capped_to_the_ceilings() {
    let r = get(&worst_case_uri());
    assert_eq!(r.status, 200);
    // 1.36 MB frames: 1 fps is the floor, and 64 MB lasts 46 frames.
    assert_eq!(r.header(CAPPED_HEADER), Some("fps=1; timeout=46"));
    let anim = animation(r);
    assert_eq!(anim.capped(), Some((1, 46)));

    let chunks = run(anim);
    assert_eq!(chunks.len(), 47, "46 frames plus the reset");
    let total: usize = chunks.iter().map(String::len).sum();
    // Frames vary a little from frame 0, which the estimate uses.
    assert!(
        (total as u64) < MAX_STREAM_BYTES * 101 / 100,
        "sent {total} bytes"
    );
    let per_frame = chunks.iter().map(String::len).max().unwrap() as u64;
    assert!(
        per_frame > MAX_BYTES_PER_SEC,
        "1 fps floor exceeds 1 MB/s here"
    );
}

#[test]
fn capped_stream_keeps_its_header_on_head() {
    let uri = worst_case_uri();
    let (path, query) = uri.split_once('?').unwrap();
    let r = handle(&Request {
        method: "HEAD",
        path,
        query: Some(query),
        ..Default::default()
    });
    assert!(matches!(r.body, Body::Empty));
    assert_eq!(r.header(CAPPED_HEADER), Some("fps=1; timeout=46"));
}

#[test]
fn large_banner_lowers_fps_before_timeout() {
    // About 270 KB a frame: 3 fps fits 1 MB/s, and 64 MB at 3 fps lasts
    // 79 s, so the default 60 s timeout stands.
    let r = get(&format!("/rainbow+block/{}", "W".repeat(200)));
    assert_eq!(r.header(CAPPED_HEADER), Some("fps=3; timeout=60"));
}
