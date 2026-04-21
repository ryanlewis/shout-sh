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
use crate::presets::is_preset;
use crate::render::is_color;

pub const MAX_TEXT_LEN: usize = 200;
pub const DEFAULT_FONT: &str = "block";
pub const DEFAULT_FPS: u32 = 10;
pub const MAX_FPS: u32 = 30;
pub const DEFAULT_TIMEOUT: u32 = 60;
pub const MAX_TIMEOUT: u32 = 300;

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
    for pair in query.split('&') {
        if pair.is_empty() {
            continue;
        }
        let (k, v) = pair.split_once('=').unwrap_or((pair, ""));
        // Valueless flags: ?animate, ?once.
        if v.is_empty() {
            match k {
                "animate" => cfg.animate = true,
                "once" => cfg.once = true,
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
    fn preset_does_not_animate_by_default() {
        assert!(!parse("/sunset/Hi", None).should_animate());
    }
}
