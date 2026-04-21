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

use crate::fonts::is_font;

pub const MAX_TEXT_LEN: usize = 200;
pub const DEFAULT_FONT: &str = "block";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenderConfig {
    pub text: String,
    pub font: String,
    pub mode: String,
    pub color: String,
    pub format: String,
}

impl Default for RenderConfig {
    fn default() -> Self {
        Self {
            text: String::new(),
            font: DEFAULT_FONT.to_string(),
            mode: String::new(),
            color: String::new(),
            format: String::new(),
        }
    }
}

const MODES: &[&str] = &["rainbow", "fire", "solid"];
const COLORS: &[&str] = &[
    "red",
    "green",
    "blue",
    "yellow",
    "cyan",
    "magenta",
    "white",
    "gray",
    "redbright",
    "greenbright",
    "bluebright",
    "yellowbright",
    "cyanbright",
    "magentabright",
    "whitebright",
];
const LAYOUTS: &[&str] = &["full", "kern", "smush"];

pub fn is_mode(tok: &str) -> bool {
    MODES.contains(&tok)
}

pub fn is_color(tok: &str) -> bool {
    COLORS.contains(&tok)
}

fn is_layout(tok: &str) -> bool {
    LAYOUTS.contains(&tok)
}

/// Parse a URL path + query into a RenderConfig.
///
/// Path shape: `/{directives}/{text}` where directives is `+`-joined tokens.
/// Single-segment paths are always text. Query params override path directives.
pub fn parse(path: &str, query: Option<&str>) -> RenderConfig {
    let mut cfg = RenderConfig::default();
    let raw = path.strip_prefix('/').unwrap_or(path);
    parse_path(raw, &mut cfg);
    if let Some(q) = query {
        apply_query(q, &mut cfg);
    }
    if cfg.text.chars().count() > MAX_TEXT_LEN {
        cfg.text = cfg.text.chars().take(MAX_TEXT_LEN).collect();
    }
    cfg
}

fn parse_path(raw: &str, cfg: &mut RenderConfig) {
    let Some(slash) = raw.find('/') else {
        cfg.text = raw.replace('+', " ");
        return;
    };
    let first = &raw[..slash];
    let rest = &raw[slash + 1..];
    if parse_directives(first, cfg) {
        cfg.text = rest.replace('+', " ");
    } else {
        cfg.text = raw.replace('+', " ");
    }
}

/// Classification order: font, mode, color, flag, layout, width. Unknown
/// tokens are ignored if at least one token matched; otherwise the whole
/// path is treated as text by the caller.
fn parse_directives(seg: &str, cfg: &mut RenderConfig) -> bool {
    let mut matched = false;
    for tok_raw in seg.split('+') {
        let tok = tok_raw.to_lowercase();
        if is_font(&tok) {
            cfg.font = tok;
            matched = true;
        } else if is_mode(&tok) {
            cfg.mode = tok;
            matched = true;
        } else if is_color(&tok) {
            cfg.color = tok;
            matched = true;
        } else if tok == "animate" || tok == "once" {
            // phase-2 flags accepted for URL compat, ignored in phase 1
            matched = true;
        } else if is_layout(&tok) {
            // layout directives were FIGlet-specific; cfonts ignores them
            matched = true;
        } else if let Some(rest) = tok.strip_prefix('w')
            && !rest.is_empty()
            && let Ok(n) = rest.parse::<u32>()
            && n > 0
        {
            // width directive accepted for URL compat; not wired to cfonts
            matched = true;
        }
    }
    matched
}

fn apply_query(query: &str, cfg: &mut RenderConfig) {
    for pair in query.split('&') {
        if pair.is_empty() {
            continue;
        }
        let (k, v) = match pair.split_once('=') {
            Some((k, v)) => (k, v),
            None => (pair, ""),
        };
        if v.is_empty() {
            continue;
        }
        match k {
            "font" => cfg.font = v.to_lowercase(),
            "mode" => cfg.mode = v.to_lowercase(),
            "color" => cfg.color = v.to_lowercase(),
            "format" => cfg.format = v.to_lowercase(),
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg(over: RenderConfig) -> RenderConfig {
        let mut want = RenderConfig::default();
        if !over.font.is_empty() {
            want.font = over.font;
        }
        want.text = over.text;
        want.mode = over.mode;
        want.color = over.color;
        want.format = over.format;
        want
    }

    #[test]
    fn single_segment_is_text() {
        assert_eq!(
            parse("/HELLO", None),
            cfg(RenderConfig {
                text: "HELLO".into(),
                ..Default::default()
            })
        );
    }

    #[test]
    fn single_segment_matching_font_name_is_text() {
        assert_eq!(
            parse("/block", None),
            cfg(RenderConfig {
                text: "block".into(),
                ..Default::default()
            })
        );
    }

    #[test]
    fn plus_replaced_with_space() {
        assert_eq!(
            parse("/Hello+World", None),
            cfg(RenderConfig {
                text: "Hello World".into(),
                ..Default::default()
            })
        );
    }

    #[test]
    fn font_directive() {
        assert_eq!(
            parse("/tiny/Hello", None),
            cfg(RenderConfig {
                font: "tiny".into(),
                text: "Hello".into(),
                ..Default::default()
            })
        );
    }

    #[test]
    fn font_directive_with_multiword_text() {
        assert_eq!(
            parse("/block/Hello+World", None),
            cfg(RenderConfig {
                font: "block".into(),
                text: "Hello World".into(),
                ..Default::default()
            })
        );
    }

    #[test]
    fn mode_rainbow() {
        assert_eq!(
            parse("/rainbow/Hi", None),
            cfg(RenderConfig {
                mode: "rainbow".into(),
                text: "Hi".into(),
                ..Default::default()
            })
        );
    }

    #[test]
    fn color_directive() {
        assert_eq!(
            parse("/red/Hi", None),
            cfg(RenderConfig {
                color: "red".into(),
                text: "Hi".into(),
                ..Default::default()
            })
        );
    }

    #[test]
    fn animate_flag_is_accepted_as_match() {
        // phase-1: animate is recognized but not surfaced in config
        assert_eq!(
            parse("/animate/Hi", None),
            cfg(RenderConfig {
                text: "Hi".into(),
                ..Default::default()
            })
        );
    }

    #[test]
    fn once_flag_is_accepted_as_match() {
        assert_eq!(
            parse("/once/Hi", None),
            cfg(RenderConfig {
                text: "Hi".into(),
                ..Default::default()
            })
        );
    }

    #[test]
    fn rainbow_plus_once() {
        assert_eq!(
            parse("/rainbow+once/Hi", None),
            cfg(RenderConfig {
                mode: "rainbow".into(),
                text: "Hi".into(),
                ..Default::default()
            })
        );
    }

    #[test]
    fn layout_directives_are_ignored_silently() {
        for l in ["full", "kern", "smush"] {
            let got = parse(&format!("/{l}/Hi"), None);
            assert_eq!(got.text, "Hi");
            assert_eq!(got.font, DEFAULT_FONT);
        }
    }

    #[test]
    fn width_directive_matches_but_is_ignored() {
        let got = parse("/w120/Hi", None);
        assert_eq!(got.text, "Hi");
    }

    #[test]
    fn combined_font_mode_animate() {
        assert_eq!(
            parse("/tiny+rainbow+animate/Hello+World", None),
            cfg(RenderConfig {
                font: "tiny".into(),
                mode: "rainbow".into(),
                text: "Hello World".into(),
                ..Default::default()
            })
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
            cfg(RenderConfig {
                font: "tiny".into(),
                mode: "rainbow".into(),
                text: "Hi".into(),
                ..Default::default()
            })
        );
    }

    #[test]
    fn unknown_first_segment_is_text() {
        assert_eq!(
            parse("/notathing/Hello", None),
            cfg(RenderConfig {
                text: "notathing/Hello".into(),
                ..Default::default()
            })
        );
    }

    #[test]
    fn unknown_token_with_known_token_still_matches() {
        assert_eq!(
            parse("/tiny+bogus/Hi", None),
            cfg(RenderConfig {
                font: "tiny".into(),
                text: "Hi".into(),
                ..Default::default()
            })
        );
    }

    #[test]
    fn width_with_non_numeric_is_no_match() {
        assert_eq!(
            parse("/wfoo/Hi", None),
            cfg(RenderConfig {
                text: "wfoo/Hi".into(),
                ..Default::default()
            })
        );
    }

    #[test]
    fn width_zero_is_no_match() {
        assert_eq!(
            parse("/w0/Hi", None),
            cfg(RenderConfig {
                text: "w0/Hi".into(),
                ..Default::default()
            })
        );
    }

    #[test]
    fn empty_path() {
        assert_eq!(
            parse("/", None),
            cfg(RenderConfig {
                text: String::new(),
                ..Default::default()
            })
        );
    }

    #[test]
    fn query_font_overrides_path_font() {
        let got = parse("/tiny/Hi", Some("font=block"));
        assert_eq!(got.font, "block");
    }

    #[test]
    fn query_mode_overrides_path_mode() {
        let got = parse("/rainbow/Hi", Some("mode=fire"));
        assert_eq!(got.mode, "fire");
    }

    #[test]
    fn query_format_json() {
        let got = parse("/Hi", Some("format=json"));
        assert_eq!(got.format, "json");
    }

    #[test]
    fn text_is_truncated_to_max_len() {
        let long: String = "a".repeat(300);
        let got = parse(&format!("/{long}"), None);
        assert_eq!(got.text.chars().count(), MAX_TEXT_LEN);
    }

    #[test]
    fn default_font_is_block() {
        let got = parse("/Hi", None);
        assert_eq!(got.font, "block");
    }
}
