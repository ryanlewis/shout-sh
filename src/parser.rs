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
use crate::render::is_color;

pub const MAX_TEXT_LEN: usize = 200;
pub const DEFAULT_FONT: &str = "block";

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
    pub json: bool,
}

impl Default for RenderConfig {
    fn default() -> Self {
        Self {
            text: String::new(),
            font: DEFAULT_FONT.to_string(),
            mode: None,
            color: String::new(),
            json: false,
        }
    }
}

const LAYOUTS: &[&str] = &["full", "kern", "smush"];

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
/// tokens are ignored when any token matched; otherwise the caller treats
/// the whole path as text.
fn parse_directives(seg: &str, cfg: &mut RenderConfig) -> bool {
    let mut matched = false;
    for tok_raw in seg.split('+') {
        let tok = tok_raw.to_lowercase();
        if is_font(&tok) {
            cfg.font = tok;
            matched = true;
        } else if let Some(m) = Mode::from_token(&tok) {
            cfg.mode = Some(m);
            matched = true;
        } else if is_color(&tok) {
            cfg.color = tok;
            matched = true;
        } else if tok == "animate" || tok == "once" || is_layout(&tok) {
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
    for pair in query.split('&') {
        if pair.is_empty() {
            continue;
        }
        let (k, v) = pair.split_once('=').unwrap_or((pair, ""));
        if v.is_empty() {
            continue;
        }
        let v = v.to_lowercase();
        match k {
            "font" => cfg.font = v,
            "mode" => cfg.mode = Mode::from_token(&v).or(cfg.mode),
            "color" => cfg.color = v,
            "format" => cfg.json = v == "json",
            _ => {}
        }
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
    fn animate_flag_matches_but_is_ignored() {
        assert_eq!(parse("/animate/Hi", None), text("Hi"));
    }

    #[test]
    fn once_flag_matches_but_is_ignored() {
        assert_eq!(parse("/once/Hi", None), text("Hi"));
    }

    #[test]
    fn rainbow_plus_once() {
        assert_eq!(
            parse("/rainbow+once/Hi", None),
            RenderConfig {
                mode: Some(Mode::Rainbow),
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
}
