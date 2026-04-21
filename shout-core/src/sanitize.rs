// shout.sh — curl-friendly ANSI banner service
// Copyright (C) 2026 Ryan Lewis
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

//! Belt-and-braces filter applied to rendered banner output.
//!
//! cfonts, our own ansi constants, and the cell emitter only need a tiny
//! vocabulary of escape sequences:
//!
//!   * `\x1b[...m`          SGR (color / bold / reset)
//!   * `\x1b[nA`            cursor up (animation re-draw)
//!   * `\x1b[H`             cursor home
//!   * `\x1b[2J`            clear screen
//!   * `\x1b[?25l`/`[?25h`  cursor visibility
//!
//! Anything else — OSC (window title, hyperlinks, OSC 52 clipboard), DCS,
//! other CSI verbs — is stripped. This defends against a cfonts regression
//! ever emitting something unexpected, so a recipient of a piped curl
//! output can't have their terminal hijacked.

/// Filter `input` down to the allowed escape-sequence vocabulary. Non-escape
/// bytes pass through untouched, so UTF-8 sequences in the glyph output
/// remain intact (multi-byte UTF-8 bytes are all ≥ 0x80 and never collide
/// with the 0x1B trigger).
pub fn sanitize_ansi(input: &str) -> String {
    let bytes = input.as_bytes();
    let mut out: Vec<u8> = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        let b = bytes[i];
        if b != 0x1b {
            out.push(b);
            i += 1;
            continue;
        }
        // ESC must be followed by `[` (CSI). Anything else — 2-byte `ESC c`,
        // `ESC ]` (OSC), `ESC P` (DCS), bare ESC — we drop the ESC byte
        // and let the following char stand as a harmless literal.
        if i + 1 >= bytes.len() || bytes[i + 1] != b'[' {
            i += 1;
            continue;
        }
        // Scan CSI body: parameter bytes 0x30..=0x3F, intermediates
        // 0x20..=0x2F, terminated by a final byte in 0x40..=0x7E.
        let start = i;
        let mut j = i + 2;
        let final_idx = loop {
            if j >= bytes.len() {
                break None;
            }
            let c = bytes[j];
            if (0x40..=0x7E).contains(&c) {
                break Some(j);
            }
            if !(0x20..=0x3F).contains(&c) {
                // malformed / interrupted — bail and drop everything
                // scanned so far
                break None;
            }
            j += 1;
        };
        match final_idx {
            Some(end) => {
                let seq = &bytes[start..=end];
                if is_allowed_csi(seq) {
                    out.extend_from_slice(seq);
                }
                i = end + 1;
            }
            None => {
                // Unterminated or malformed — drop the ESC [ prefix and
                // resume scanning from the next byte.
                i += 2;
            }
        }
    }
    // Safe: we only ever copied whole bytes. Non-ESC bytes (including all
    // UTF-8 continuation bytes) pass through as-is, and allowed CSI
    // sequences are pure ASCII.
    String::from_utf8(out).expect("filter preserves utf-8")
}

fn is_allowed_csi(seq: &[u8]) -> bool {
    debug_assert!(seq.len() >= 3 && seq[0] == 0x1b && seq[1] == b'[');
    let final_b = *seq.last().unwrap();
    let params = &seq[2..seq.len() - 1];
    match final_b {
        // SGR: digits and `;` only. Blocks private-marker forms like
        // `ESC [ > 0 m` that some terminals treat as device queries.
        b'm' => params.iter().all(|&c| c.is_ascii_digit() || c == b';'),
        // Cursor up — digits only (or empty for default 1).
        b'A' => params.iter().all(|&c| c.is_ascii_digit()),
        // Cursor home — no params.
        b'H' => params.is_empty(),
        // Clear screen — allow only the "entire screen" variant.
        b'J' => params == b"2",
        // Cursor visibility — `?25l` hide, `?25h` show.
        b'l' | b'h' => params == b"?25",
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sgr_color_passes() {
        let s = "\x1b[31mhi\x1b[39m";
        assert_eq!(sanitize_ansi(s), s);
    }

    #[test]
    fn truecolor_sgr_passes() {
        let s = "\x1b[38;2;10;20;30m█\x1b[39m";
        assert_eq!(sanitize_ansi(s), s);
    }

    #[test]
    fn cursor_ops_pass() {
        for s in [
            "\x1b[2J",
            "\x1b[H",
            "\x1b[?25l",
            "\x1b[?25h",
            "\x1b[5A",
            "\x1b[0m",
        ] {
            assert_eq!(sanitize_ansi(s), s, "expected {s:?} to pass");
        }
    }

    #[test]
    fn osc_hyperlink_stripped() {
        // OSC 8 hyperlink — the biggest real-world threat: masks a
        // malicious URL under innocent-looking display text.
        let s = "\x1b]8;;http://evil.example/\x1b\\LOOKHERE\x1b]8;;\x1b\\";
        let out = sanitize_ansi(s);
        assert!(!out.contains('\x1b'), "no escapes should survive: {out:?}");
        assert!(out.contains("LOOKHERE"));
    }

    #[test]
    fn osc_52_clipboard_stripped() {
        // OSC 52 — write-to-clipboard. Terminal.app ignores it, but
        // iTerm/xterm/kitty honour it. Must never leak.
        let s = "\x1b]52;c;cm0=\x1b\\";
        assert!(!sanitize_ansi(s).contains('\x1b'));
    }

    #[test]
    fn dcs_stripped() {
        let s = "\x1bP0;0|17/ab\x1b\\";
        assert!(!sanitize_ansi(s).contains('\x1b'));
    }

    #[test]
    fn private_sgr_stripped() {
        // `ESC [ > 0 m` is a private-marker SGR. Some terminals treat
        // private-marker CSIs as queries that reply on stdin — a
        // response-injection vector if piped somewhere unexpected.
        let s = "\x1b[>0m";
        assert_eq!(sanitize_ansi(s), "");
    }

    #[test]
    fn device_status_report_stripped() {
        // ESC [ 6 n — "report cursor position". Reply comes back on stdin.
        let s = "\x1b[6n";
        assert_eq!(sanitize_ansi(s), "");
    }

    #[test]
    fn unterminated_csi_stripped() {
        // ESC [ with no final byte — drop prefix, keep the tail.
        assert_eq!(sanitize_ansi("\x1b[31"), "31");
    }

    #[test]
    fn bare_esc_stripped() {
        assert_eq!(sanitize_ansi("a\x1bb"), "ab");
    }

    #[test]
    fn plain_text_untouched() {
        assert_eq!(sanitize_ansi("hello world\n"), "hello world\n");
    }

    #[test]
    fn utf8_glyphs_preserved() {
        // Box-drawing chars used by cfonts are multi-byte UTF-8.
        let s = "\x1b[31m█╗\x1b[39m";
        assert_eq!(sanitize_ansi(s), s);
    }

    #[test]
    fn only_clear_entire_screen_allowed() {
        // `\x1b[0J` (clear from cursor down) and `\x1b[1J` (up) would let
        // an attacker scrub scrollback. Only `2J` (whole screen) passes.
        assert_eq!(sanitize_ansi("\x1b[0J"), "");
        assert_eq!(sanitize_ansi("\x1b[1J"), "");
        assert_eq!(sanitize_ansi("\x1b[2J"), "\x1b[2J");
    }

    #[test]
    fn only_cursor_visibility_modes_allowed() {
        // `\x1b[?1049h` is the xterm alt-screen switch — would clear the
        // user's scrollback view. Must not pass.
        assert_eq!(sanitize_ansi("\x1b[?1049h"), "");
        assert_eq!(sanitize_ansi("\x1b[?25h"), "\x1b[?25h");
    }
}
