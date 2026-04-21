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

use percent_encoding::percent_decode_str;

use crate::fonts::is_font;
use crate::presets::is_preset;
use crate::render::is_color;

/// Percent-decode a URL segment as UTF-8, falling back to the raw input on
/// invalid sequences. We decode text payloads (not the directive structure)
/// so `%7C` → `|`, `%20` → space, etc. reach cfonts as intended.
fn decode(s: &str) -> String {
    percent_decode_str(s).decode_utf8_lossy().into_owned()
}

pub const MAX_TEXT_LEN: usize = 200;
pub const DEFAULT_FONT: &str = "block";
pub const DEFAULT_FPS: u32 = 10;
pub const MAX_FPS: u32 = 30;
pub const DEFAULT_TIMEOUT: u32 = 60;
pub const MAX_TIMEOUT: u32 = 300;

/// Hard cap on path + query combined. The fallback handler rejects anything
/// larger with 414 so we never alloc / decode multi-KB user input.
pub const MAX_URL_LEN: usize = 512;
/// Cap on `{name}` path params on `/fonts/{name}` and `/presets/{name}`.
pub const MAX_PARAM_LEN: usize = 64;
/// Max `+`-separated tokens we'll classify in a directive segment.
pub const MAX_DIRECTIVE_TOKENS: usize = 16;
/// Max characters in a single directive token (pre-lowercase). Anything
/// longer can't be a real directive so we skip it without allocating.
pub const MAX_TOKEN_LEN: usize = 32;
/// Max `&`-separated pairs we'll honor in a query string.
pub const MAX_QUERY_PAIRS: usize = 32;
/// Max characters in a single query value.
pub const MAX_QUERY_VALUE_LEN: usize = 64;
/// Max `|` line-break characters in the text payload. cfonts turns each `|`
/// into an extra banner row, so without a cap a 200-char text could produce
/// hundreds of glyph rows.
pub const MAX_LINE_BREAKS: usize = 8;

pub const DEFAULT_LETTER_SPACING: u16 = 1;
pub const MAX_LETTER_SPACING: u16 = 10;
/// Blank rows above and below the banner. Replaces cfonts' hardcoded
/// 2-row padding (which is toggled on/off by `spaceless`) with a continuous
/// knob. Default 2 to match cfonts' non-spaceless behavior.
pub const DEFAULT_PADDING: u16 = 2;
pub const MAX_PADDING: u16 = 10;
/// 0 means "no limit" — cfonts treats it that way too.
pub const MAX_MAX_LENGTH: u16 = 200;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Solid,
    Rainbow,
    Fire,
}

impl Mode {
    pub fn from_token(tok: &str) -> Option<Self> {
        Some(match tok {
            "solid" => Self::Solid,
            "rainbow" => Self::Rainbow,
            "fire" => Self::Fire,
            _ => return None,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenderConfig {
    pub text: String,
    pub font: String,
    pub mode: Option<Mode>,
    pub color: String,
    pub preset: String,
    pub json: bool,
    pub animate: bool,
    pub once: bool,
    pub fps: u32,
    pub timeout: u32,
    pub letter_spacing: u16,
    pub max_length: u16,
    pub padding: u16,
    pub background: String,
    /// Internal: when true, render.rs drives cfonts with `Env::Browser` so
    /// its terminal-width-based wrapping is lifted (cfonts falls back to 80
    /// cols when there's no tty, which is always the case in wasm). Output
    /// is normalized back to the SGR format the cell pipeline expects.
    pub browser: bool,
}

impl Default for RenderConfig {
    fn default() -> Self {
        Self {
            text: String::new(),
            font: DEFAULT_FONT.to_string(),
            mode: None,
            color: String::new(),
            preset: String::new(),
            json: false,
            animate: false,
            once: false,
            fps: DEFAULT_FPS,
            timeout: DEFAULT_TIMEOUT,
            letter_spacing: DEFAULT_LETTER_SPACING,
            max_length: 0,
            padding: DEFAULT_PADDING,
            background: String::new(),
            browser: false,
        }
    }
}

impl RenderConfig {
    /// Animation is on by default for rainbow/fire; off for solid/bare color.
    /// `once` and JSON always force static. `animate` overrides the default.
    pub fn should_animate(&self) -> bool {
        if self.once || self.json {
            return false;
        }
        if self.animate {
            return true;
        }
        matches!(self.mode, Some(Mode::Rainbow) | Some(Mode::Fire))
    }
}

const LAYOUTS: &[&str] = &["full", "kern", "smush"];

pub fn parse(path: &str, query: Option<&str>) -> RenderConfig {
    let mut cfg = RenderConfig::default();
    let raw = path.strip_prefix('/').unwrap_or(path);
    // Belt-and-braces: the server layer rejects oversize URLs with 414, but
    // other callers (wasm, tests) hit `parse` directly. Truncate the raw
    // path so nothing downstream has to cope with megabytes of input.
    let raw = truncate_chars(raw, MAX_URL_LEN);
    parse_path(&raw, &mut cfg);
    if let Some(q) = query {
        let q = truncate_chars(q, MAX_URL_LEN);
        apply_query(&q, &mut cfg);
    }
    cfg.text = sanitize_text(&cfg.text);
    if cfg.text.chars().count() > MAX_TEXT_LEN {
        cfg.text = cfg.text.chars().take(MAX_TEXT_LEN).collect();
    }
    cfg
}

fn truncate_chars(s: &str, max: usize) -> String {
    if s.len() <= max {
        return s.to_string();
    }
    s.chars().take(max).collect()
}

/// Drop ASCII control characters (incl. ESC) and unicode bidi-override
/// formatting chars before handing text to cfonts. Without this a caller
/// could embed `%1B[...` to smuggle terminal escape sequences, or bidi
/// overrides (U+202A..202E, U+2066..2069) to visually reorder characters
/// in a way that disguises the rendered banner when shared elsewhere.
fn sanitize_text(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut breaks = 0usize;
    for c in s.chars() {
        if c == '|' {
            if breaks >= MAX_LINE_BREAKS {
                continue;
            }
            breaks += 1;
            out.push(c);
            continue;
        }
        if c.is_control() || is_bidi_formatting(c) {
            continue;
        }
        out.push(c);
    }
    out
}

/// Unicode directional formatting / isolate code points. These don't count
/// as control characters by `char::is_control()` but can flip the visible
/// order of subsequent text in many terminals.
fn is_bidi_formatting(c: char) -> bool {
    matches!(
        c,
        '\u{202A}'..='\u{202E}'  // LRE, RLE, PDF, LRO, RLO
        | '\u{2066}'..='\u{2069}' // LRI, RLI, FSI, PDI
        | '\u{FEFF}'             // BOM / zero-width no-break space
    )
}

fn parse_path(raw: &str, cfg: &mut RenderConfig) {
    let Some(slash) = raw.find('/') else {
        cfg.text = decode(&raw.replace('+', " "));
        return;
    };
    let first = &raw[..slash];
    let rest = &raw[slash + 1..];
    if parse_directives(first, cfg) {
        cfg.text = decode(&rest.replace('+', " "));
    } else {
        cfg.text = decode(&raw.replace('+', " "));
    }
}

/// Classification order: font, mode, color, flag, layout, width. Unknown
/// tokens are ignored when any token matched; otherwise the caller treats
/// the whole path as text.
fn parse_directives(seg: &str, cfg: &mut RenderConfig) -> bool {
    let mut matched = false;
    for tok_raw in seg.split('+').take(MAX_DIRECTIVE_TOKENS) {
        // Skip oversize tokens without lowercasing — no real directive is
        // this long, so it's either junk or an attempt to blow up alloc.
        if tok_raw.len() > MAX_TOKEN_LEN {
            continue;
        }
        let tok = tok_raw.to_lowercase();
        if is_font(&tok) {
            cfg.font = tok;
            matched = true;
        } else if let Some(m) = Mode::from_token(&tok) {
            cfg.mode = Some(m);
            matched = true;
        } else if is_preset(&tok) {
            cfg.preset = tok;
            matched = true;
        } else if is_color(&tok) {
            cfg.color = tok;
            matched = true;
        } else if tok == "animate" {
            cfg.animate = true;
            matched = true;
        } else if tok == "once" {
            cfg.once = true;
            matched = true;
        } else if tok == "spaceless" {
            // Legacy alias: `spaceless` === `padding=0`.
            cfg.padding = 0;
            matched = true;
        } else if is_layout(&tok) {
            matched = true;
        } else if let Some(rest) = tok.strip_prefix('w')
            && !rest.is_empty()
            && let Ok(n) = rest.parse::<u32>()
            && n > 0
        {
            matched = true;
        }
    }
    matched
}

fn is_layout(tok: &str) -> bool {
    LAYOUTS.contains(&tok)
}

fn apply_query(query: &str, cfg: &mut RenderConfig) {
    for pair in query.split('&').take(MAX_QUERY_PAIRS) {
        if pair.is_empty() {
            continue;
        }
        let (k, v) = pair.split_once('=').unwrap_or((pair, ""));
        // Clip each value before lowercasing/storing so an attacker can't
        // balloon `font=<megabytes>` into memory through any branch below.
        // `floor_char_boundary` is unstable — walk back to a UTF-8 boundary.
        let mut end = v.len().min(MAX_QUERY_VALUE_LEN);
        while end > 0 && !v.is_char_boundary(end) {
            end -= 1;
        }
        let v = &v[..end];
        // Valueless flags: ?animate, ?once.
        if v.is_empty() {
            match k {
                "animate" => cfg.animate = true,
                "once" => cfg.once = true,
                "spaceless" => cfg.padding = 0,
                _ => {}
            }
            continue;
        }
        let v = v.to_lowercase();
        match k {
            "font" => cfg.font = v,
            "mode" => cfg.mode = Mode::from_token(&v).or(cfg.mode),
            "color" => cfg.color = v,
            "preset" => cfg.preset = v,
            "format" => cfg.json = v == "json",
            "fps" => {
                if let Ok(n) = v.parse::<u32>() {
                    cfg.fps = clamp_fps(n);
                }
            }
            "timeout" => {
                if let Ok(n) = v.parse::<u32>() {
                    cfg.timeout = clamp_timeout(n);
                }
            }
            "spacing" | "letter-spacing" | "letter_spacing" | "ls" => {
                if let Ok(n) = v.parse::<u16>() {
                    cfg.letter_spacing = n.min(MAX_LETTER_SPACING);
                }
            }
            "padding" | "pad" => {
                if let Ok(n) = v.parse::<u16>() {
                    cfg.padding = n.min(MAX_PADDING);
                }
            }
            "maxlength" | "max-length" | "max_length" | "ml" => {
                if let Ok(n) = v.parse::<u16>() {
                    cfg.max_length = n.min(MAX_MAX_LENGTH);
                }
            }
            "bg" | "background" => cfg.background = v,
            "spaceless" if v != "0" && v != "false" => cfg.padding = 0,
            _ => {}
        }
    }
}

fn clamp_fps(n: u32) -> u32 {
    if n == 0 { DEFAULT_FPS } else { n.min(MAX_FPS) }
}

fn clamp_timeout(n: u32) -> u32 {
    if n == 0 {
        DEFAULT_TIMEOUT
    } else {
        n.min(MAX_TIMEOUT)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn text(s: &str) -> RenderConfig {
        RenderConfig {
            text: s.into(),
            ..Default::default()
        }
    }

    #[test]
    fn single_segment_is_text() {
        assert_eq!(parse("/HELLO", None), text("HELLO"));
    }

    #[test]
    fn single_segment_matching_font_name_is_text() {
        assert_eq!(parse("/block", None), text("block"));
    }

    #[test]
    fn plus_replaced_with_space() {
        assert_eq!(parse("/Hello+World", None), text("Hello World"));
    }

    #[test]
    fn font_directive() {
        assert_eq!(
            parse("/tiny/Hello", None),
            RenderConfig {
                font: "tiny".into(),
                text: "Hello".into(),
                ..Default::default()
            }
        );
    }

    #[test]
    fn font_directive_with_multiword_text() {
        assert_eq!(
            parse("/block/Hello+World", None),
            RenderConfig {
                font: "block".into(),
                text: "Hello World".into(),
                ..Default::default()
            }
        );
    }

    #[test]
    fn mode_rainbow() {
        assert_eq!(
            parse("/rainbow/Hi", None),
            RenderConfig {
                mode: Some(Mode::Rainbow),
                text: "Hi".into(),
                ..Default::default()
            }
        );
    }

    #[test]
    fn color_directive() {
        assert_eq!(
            parse("/red/Hi", None),
            RenderConfig {
                color: "red".into(),
                text: "Hi".into(),
                ..Default::default()
            }
        );
    }

    #[test]
    fn animate_flag_sets_bool() {
        assert_eq!(
            parse("/animate/Hi", None),
            RenderConfig {
                animate: true,
                text: "Hi".into(),
                ..Default::default()
            }
        );
    }

    #[test]
    fn once_flag_sets_bool() {
        assert_eq!(
            parse("/once/Hi", None),
            RenderConfig {
                once: true,
                text: "Hi".into(),
                ..Default::default()
            }
        );
    }

    #[test]
    fn rainbow_plus_once() {
        assert_eq!(
            parse("/rainbow+once/Hi", None),
            RenderConfig {
                mode: Some(Mode::Rainbow),
                once: true,
                text: "Hi".into(),
                ..Default::default()
            }
        );
    }

    #[test]
    fn layout_directives_ignored_silently() {
        for l in ["full", "kern", "smush"] {
            let got = parse(&format!("/{l}/Hi"), None);
            assert_eq!(got.text, "Hi");
            assert_eq!(got.font, DEFAULT_FONT);
        }
    }

    #[test]
    fn width_directive_matches_but_is_ignored() {
        assert_eq!(parse("/w120/Hi", None).text, "Hi");
    }

    #[test]
    fn combined_font_mode_animate() {
        assert_eq!(
            parse("/tiny+rainbow+animate/Hello+World", None),
            RenderConfig {
                font: "tiny".into(),
                mode: Some(Mode::Rainbow),
                animate: true,
                text: "Hello World".into(),
                ..Default::default()
            }
        );
    }

    #[test]
    fn combined_font_width_layout() {
        let got = parse("/tiny+w80+kern/Hi", None);
        assert_eq!(got.font, "tiny");
        assert_eq!(got.text, "Hi");
    }

    #[test]
    fn directives_are_case_insensitive() {
        assert_eq!(
            parse("/TINY+RAINBOW/Hi", None),
            RenderConfig {
                font: "tiny".into(),
                mode: Some(Mode::Rainbow),
                text: "Hi".into(),
                ..Default::default()
            }
        );
    }

    #[test]
    fn unknown_first_segment_is_text() {
        assert_eq!(parse("/notathing/Hello", None), text("notathing/Hello"));
    }

    #[test]
    fn unknown_token_with_known_token_still_matches() {
        assert_eq!(
            parse("/tiny+bogus/Hi", None),
            RenderConfig {
                font: "tiny".into(),
                text: "Hi".into(),
                ..Default::default()
            }
        );
    }

    #[test]
    fn width_with_non_numeric_is_no_match() {
        assert_eq!(parse("/wfoo/Hi", None), text("wfoo/Hi"));
    }

    #[test]
    fn width_zero_is_no_match() {
        assert_eq!(parse("/w0/Hi", None), text("w0/Hi"));
    }

    #[test]
    fn empty_path() {
        assert_eq!(parse("/", None), RenderConfig::default());
    }

    #[test]
    fn query_font_overrides_path_font() {
        assert_eq!(parse("/tiny/Hi", Some("font=block")).font, "block");
    }

    #[test]
    fn query_mode_overrides_path_mode() {
        assert_eq!(
            parse("/rainbow/Hi", Some("mode=fire")).mode,
            Some(Mode::Fire)
        );
    }

    #[test]
    fn query_format_json() {
        assert!(parse("/Hi", Some("format=json")).json);
    }

    #[test]
    fn text_is_truncated_to_max_len() {
        let long: String = "a".repeat(300);
        assert_eq!(
            parse(&format!("/{long}"), None).text.chars().count(),
            MAX_TEXT_LEN
        );
    }

    #[test]
    fn default_font_is_block() {
        assert_eq!(parse("/Hi", None).font, "block");
    }

    #[test]
    fn default_fps_and_timeout() {
        let cfg = parse("/Hi", None);
        assert_eq!(cfg.fps, DEFAULT_FPS);
        assert_eq!(cfg.timeout, DEFAULT_TIMEOUT);
    }

    #[test]
    fn fps_clamped_to_max() {
        assert_eq!(parse("/Hi", Some("fps=9999")).fps, MAX_FPS);
    }

    #[test]
    fn fps_zero_falls_back_to_default() {
        assert_eq!(parse("/Hi", Some("fps=0")).fps, DEFAULT_FPS);
    }

    #[test]
    fn fps_custom_accepted() {
        assert_eq!(parse("/Hi", Some("fps=20")).fps, 20);
    }

    #[test]
    fn timeout_clamped_to_max() {
        assert_eq!(parse("/Hi", Some("timeout=9999")).timeout, MAX_TIMEOUT);
    }

    #[test]
    fn timeout_zero_falls_back_to_default() {
        assert_eq!(parse("/Hi", Some("timeout=0")).timeout, DEFAULT_TIMEOUT);
    }

    #[test]
    fn query_animate_valueless() {
        assert!(parse("/Hi", Some("animate")).animate);
    }

    #[test]
    fn query_once_valueless() {
        assert!(parse("/Hi", Some("once")).once);
    }

    #[test]
    fn should_animate_rainbow_default() {
        assert!(parse("/rainbow/Hi", None).should_animate());
    }

    #[test]
    fn should_animate_fire_default() {
        assert!(parse("/fire/Hi", None).should_animate());
    }

    #[test]
    fn should_not_animate_solid_default() {
        assert!(!parse("/solid/Hi", None).should_animate());
        assert!(!parse("/red/Hi", None).should_animate());
        assert!(!parse("/Hi", None).should_animate());
    }

    #[test]
    fn once_forces_static() {
        assert!(!parse("/rainbow+once/Hi", None).should_animate());
    }

    #[test]
    fn json_forces_static() {
        assert!(!parse("/rainbow/Hi", Some("format=json")).should_animate());
    }

    #[test]
    fn animate_overrides_default() {
        assert!(parse("/solid+animate/Hi", None).should_animate());
    }

    #[test]
    fn preset_directive() {
        assert_eq!(
            parse("/sunset/Hi", None),
            RenderConfig {
                preset: "sunset".into(),
                text: "Hi".into(),
                ..Default::default()
            }
        );
    }

    #[test]
    fn preset_and_bare_color_last_wins() {
        // Grammar-level: classifier assigns both, render_config prefers preset.
        let cfg = parse("/sunset+red/Hi", None);
        assert_eq!(cfg.preset, "sunset");
        assert_eq!(cfg.color, "red");
    }

    #[test]
    fn preset_via_query() {
        assert_eq!(parse("/Hi", Some("preset=neon")).preset, "neon");
    }

    #[test]
    fn percent_encoded_pipe_decodes_to_newline_bar() {
        // `|` is cfonts' line-break directive. Accept `%7C` as equivalent so
        // shells that can't send a literal `|` still get multi-line output.
        assert_eq!(parse("/HELLO%7Cthere", None).text, "HELLO|there");
    }

    #[test]
    fn percent_encoded_space_decodes() {
        assert_eq!(parse("/Hello%20World", None).text, "Hello World");
    }

    #[test]
    fn percent_encoded_in_text_after_directive() {
        assert_eq!(parse("/tiny/HI%7Cthere", None).text, "HI|there");
    }

    #[test]
    fn control_chars_stripped_from_text() {
        // %1B is ESC; raw escapes must not reach cfonts (terminal injection).
        let cfg = parse("/%1B%5B31mPWNED", None);
        assert!(!cfg.text.contains('\x1b'));
        assert!(!cfg.text.chars().any(|c| c.is_control()));
    }

    #[test]
    fn bidi_override_stripped_from_text() {
        // U+202E (RLO) flips the visible order of following chars.
        let cfg = parse("/abc\u{202E}def", None);
        assert!(!cfg.text.contains('\u{202E}'));
    }

    #[test]
    fn pipe_line_breaks_capped() {
        let many = "a".to_string() + &"|a".repeat(50);
        let cfg = parse(&format!("/{many}"), None);
        let breaks = cfg.text.chars().filter(|c| *c == '|').count();
        assert!(
            breaks <= MAX_LINE_BREAKS,
            "got {breaks} pipes, want <= {MAX_LINE_BREAKS}"
        );
    }

    #[test]
    fn oversize_directive_token_ignored() {
        // A huge non-matching token next to a real one shouldn't alloc the
        // lowercase and shouldn't prevent `tiny` from matching.
        // Just over MAX_TOKEN_LEN — enough to trigger the skip-without-
        // lowercasing path, without pushing the whole URL past MAX_URL_LEN.
        let huge = "z".repeat(MAX_TOKEN_LEN * 4);
        let path = format!("/tiny+{huge}/Hi");
        let cfg = parse(&path, None);
        assert_eq!(cfg.font, "tiny");
        assert_eq!(cfg.text, "Hi");
    }

    #[test]
    fn directive_token_count_capped() {
        // More than MAX_DIRECTIVE_TOKENS junk tokens before a real one —
        // everything past the cap is dropped. Real directive at position
        // MAX_DIRECTIVE_TOKENS+1 should NOT be applied.
        let junk = "bogus+".repeat(MAX_DIRECTIVE_TOKENS);
        let path = format!("/{junk}tiny/Hi");
        let cfg = parse(&path, None);
        assert_eq!(cfg.font, DEFAULT_FONT, "late `tiny` should be dropped");
    }

    #[test]
    fn oversize_query_value_truncated() {
        // `font=<huge>` should never cause megabytes of allocation. We can't
        // easily observe the clip externally, but parsing must not hang or
        // panic and the bogus font falls through as the default name space.
        let huge = "z".repeat(10_000);
        let q = format!("font={huge}");
        let cfg = parse("/Hi", Some(&q));
        assert!(cfg.font.len() <= MAX_QUERY_VALUE_LEN);
    }

    #[test]
    fn query_pair_count_capped() {
        // Past MAX_QUERY_PAIRS we stop reading, so the final `fps=25` is
        // ignored and the default remains.
        let mut q = String::new();
        for _ in 0..MAX_QUERY_PAIRS {
            q.push_str("x=1&");
        }
        q.push_str("fps=25");
        let cfg = parse("/Hi", Some(&q));
        assert_eq!(cfg.fps, DEFAULT_FPS);
    }

    #[test]
    fn oversize_url_truncated_safely() {
        let huge = "a".repeat(MAX_URL_LEN * 4);
        let cfg = parse(&format!("/{huge}"), None);
        assert!(cfg.text.chars().count() <= MAX_TEXT_LEN);
    }

    #[test]
    fn preset_does_not_animate_by_default() {
        assert!(!parse("/sunset/Hi", None).should_animate());
    }
}
