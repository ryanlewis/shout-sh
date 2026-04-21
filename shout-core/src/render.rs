// shout.sh — curl-friendly ANSI banner service
// Copyright (C) 2026 Ryan Lewis
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

use cfonts::{BgColors, Colors, Env, Options, Rgb, render};

use crate::fonts::{self, resolve};
use crate::parser::{Mode, RenderConfig};
use crate::presets;
use crate::sanitize::sanitize_ansi;
use crate::sgr::{self, Cell};
use crate::shader::{Filter, Rainbow, SLOT_SENTINELS};

#[derive(Debug, PartialEq, Eq)]
pub enum RenderError {
    UnknownFont,
    UnknownColor,
    UnknownPreset,
    EmptyText,
}

impl RenderError {
    pub fn message(&self) -> &'static str {
        match self {
            Self::UnknownFont => "font not found. try `curl shout.sh/fonts`.",
            Self::UnknownColor => {
                "color not found. try `red`, `green`, `blue`, `yellow`, `cyan`, `magenta`, `white`, `gray`, or a `*bright` variant."
            }
            Self::UnknownPreset => "preset not found. try `curl shout.sh/presets`.",
            Self::EmptyText => "nothing to shout about. type something.",
        }
    }
}

fn color_enum(name: &str) -> Option<Colors> {
    Some(match name {
        "red" => Colors::Red,
        "green" => Colors::Green,
        "blue" => Colors::Blue,
        "yellow" => Colors::Yellow,
        "cyan" => Colors::Cyan,
        "magenta" => Colors::Magenta,
        "white" => Colors::White,
        "gray" => Colors::Gray,
        "redbright" => Colors::RedBright,
        "greenbright" => Colors::GreenBright,
        "bluebright" => Colors::BlueBright,
        "yellowbright" => Colors::YellowBright,
        "cyanbright" => Colors::CyanBright,
        "magentabright" => Colors::MagentaBright,
        "whitebright" => Colors::WhiteBright,
        _ => return None,
    })
}

pub fn is_color(name: &str) -> bool {
    color_enum(name).is_some()
}

fn bg_color_enum(name: &str) -> Option<BgColors> {
    Some(match name {
        "" | "transparent" => BgColors::Transparent,
        "black" => BgColors::Black,
        "red" => BgColors::Red,
        "green" => BgColors::Green,
        "blue" => BgColors::Blue,
        "yellow" => BgColors::Yellow,
        "cyan" => BgColors::Cyan,
        "magenta" => BgColors::Magenta,
        "white" => BgColors::White,
        "gray" => BgColors::Gray,
        "redbright" => BgColors::RedBright,
        "greenbright" => BgColors::GreenBright,
        "bluebright" => BgColors::BlueBright,
        "yellowbright" => BgColors::YellowBright,
        "cyanbright" => BgColors::CyanBright,
        "magentabright" => BgColors::MagentaBright,
        "whitebright" => BgColors::WhiteBright,
        _ => return None,
    })
}

pub fn is_bg_color(name: &str) -> bool {
    bg_color_enum(name).is_some()
}

/// For shader-driven modes: paint each of the font's slots with a distinct
/// sentinel RGB so the shader can see which slot a cell came from. For
/// single-slot fonts there's nothing to differentiate, so `fallback` is
/// used instead (typically `Colors::White` for rainbow).
fn apply_slot_sentinels(font_name: &str, opts: &mut Options, fallback: Colors) {
    let slots = fonts::color_count(font_name);
    if slots >= 2 {
        opts.colors = (0..slots)
            .map(|i| {
                let (r, g, b) = SLOT_SENTINELS[i.min(SLOT_SENTINELS.len() - 1)];
                Colors::Rgb(Rgb::Val(r, g, b))
            })
            .collect();
    } else {
        opts.colors = vec![fallback];
    }
}

fn hex_to_rgb(hex: &str) -> Rgb {
    let h = hex.trim_start_matches('#');
    let r = u8::from_str_radix(h.get(0..2).unwrap_or("00"), 16).unwrap_or(0);
    let g = u8::from_str_radix(h.get(2..4).unwrap_or("00"), 16).unwrap_or(0);
    let b = u8::from_str_radix(h.get(4..6).unwrap_or("00"), 16).unwrap_or(0);
    Rgb::Val(r, g, b)
}

/// Map a preset onto cfonts. Multi-slot fonts (3d, block, chrome, ...) get
/// one solid color per slot — matches `cfonts -c A,B` and keeps slot 1 and
/// slot 2 visually distinct. Single-slot fonts fall back to the transition
/// gradient so the palette still reads across the text.
fn apply_preset(font_name: &str, preset_name: &str, opts: &mut Options) -> Result<(), RenderError> {
    let preset = presets::resolve(preset_name).ok_or(RenderError::UnknownPreset)?;
    let slots = fonts::color_count(font_name);
    if slots >= 2 {
        let mut stops: Vec<&str> = preset.stops.iter().copied().take(slots).collect();
        while stops.len() < slots {
            stops.push(*stops.last().unwrap());
        }
        opts.colors = stops
            .into_iter()
            .map(|s| Colors::Rgb(hex_to_rgb(s)))
            .collect();
    } else {
        let mut stops: Vec<String> = preset.stops.iter().map(|s| (*s).into()).collect();
        if stops.len() < 2 {
            // transition_gradient expects at least 2 anchors.
            stops.push(stops[0].clone());
        }
        opts.gradient = stops;
        opts.transition_gradient = true;
    }
    Ok(())
}

/// Raw cfonts render as String. For Rainbow mode, renders with a neutral
/// white base so the shader pipeline can recolor every non-space glyph.
fn render_raw(cfg: &RenderConfig) -> Result<String, RenderError> {
    if cfg.text.is_empty() {
        return Err(RenderError::EmptyText);
    }

    let font = resolve(&cfg.font).ok_or(RenderError::UnknownFont)?;

    // cfonts' own `spaceless=false` hardcodes 2 blank rows above + below.
    // Always pass `spaceless=true` so we can add `cfg.padding` rows ourselves
    // — gives the caller a continuous 0..=MAX_PADDING knob.
    let mut opts = Options {
        text: cfg.text.clone(),
        font,
        letter_spacing: cfg.letter_spacing,
        max_length: cfg.max_length,
        spaceless: true,
        background: bg_color_enum(&cfg.background).unwrap_or(BgColors::Transparent),
        env: if cfg.browser { Env::Browser } else { Env::Cli },
        ..Options::default()
    };

    match cfg.mode {
        None => {
            if !cfg.preset.is_empty() {
                apply_preset(&cfg.font, &cfg.preset, &mut opts)?;
            } else if !cfg.color.is_empty() {
                opts.colors = vec![color_enum(&cfg.color).ok_or(RenderError::UnknownColor)?];
            }
        }
        Some(Mode::Solid) => {
            if !cfg.preset.is_empty() {
                apply_preset(&cfg.font, &cfg.preset, &mut opts)?;
            } else {
                let name = if cfg.color.is_empty() {
                    "white"
                } else {
                    cfg.color.as_str()
                };
                opts.colors = vec![color_enum(name).ok_or(RenderError::UnknownColor)?];
            }
        }
        Some(Mode::Rainbow) => {
            // Multi-slot fonts get per-slot sentinel colors so the shader can
            // differentiate front/shadow layers. Single-slot fonts fall back
            // to a neutral white base (shader recolors every non-space cell).
            // Borders emit bare and are picked up via `char != ' '`.
            apply_slot_sentinels(&cfg.font, &mut opts, Colors::White);
        }
        Some(Mode::Fire) => {
            // Same pattern as rainbow — multi-slot fonts get sentinels, so
            // the Fire shader can render slot 1+ as dim embers behind the
            // flame. Single-slot fonts use the legacy gradient path.
            if fonts::color_count(&cfg.font) >= 2 {
                apply_slot_sentinels(&cfg.font, &mut opts, Colors::White);
            } else {
                // Named-color gradients panic cfonts (see spike-findings); always pass hex.
                opts.gradient = vec!["#ff0000".into(), "#ff9900".into(), "#ffff00".into()];
                opts.transition_gradient = true;
            }
        }
    }

    let raw = render(opts).text;
    let normalized = if cfg.browser {
        browser_to_sgr(&raw)
    } else {
        // Belt-and-braces against a future cfonts regression emitting
        // anything beyond plain SGR. `browser_to_sgr` already constructs
        // escapes from scratch, so it skips this filter.
        sanitize_ansi(&raw)
    };
    Ok(pad_vertically(&normalized, cfg.padding))
}

/// Convert cfonts `Env::Browser` output into the SGR-bearing text our cell
/// pipeline expects. Browser output wraps everything in a `<div>…</div>`,
/// separates rows with `<br>\n`, and colors letters via
/// `<span style="color:#rrggbb">…</span>`. We strip the wrapper, remove the
/// `<br>` markers, and rewrite the spans as truecolor SGR escapes. This lets
/// us opt into `Env::Browser`'s 0xFFFF wrap width without adopting its HTML
/// output format.
fn browser_to_sgr(s: &str) -> String {
    let after_open = s.split_once('>').map(|(_, rest)| rest).unwrap_or(s);
    let body = after_open.strip_suffix("</div>").unwrap_or(after_open);

    use std::fmt::Write as _;
    let mut out = String::with_capacity(body.len());
    let mut rest = body;
    while !rest.is_empty() {
        if let Some(lt) = rest.find('<') {
            out.push_str(&rest[..lt]);
            rest = &rest[lt..];
            let end = match rest.find('>') {
                Some(e) => e,
                None => {
                    out.push_str(rest);
                    break;
                }
            };
            let tag = &rest[..=end];
            if let Some(hex) = tag
                .strip_prefix("<span style=\"color:#")
                .and_then(|r| r.strip_suffix("\">"))
                && hex.len() == 6
            {
                let r = u8::from_str_radix(&hex[0..2], 16).unwrap_or(0);
                let g = u8::from_str_radix(&hex[2..4], 16).unwrap_or(0);
                let b = u8::from_str_radix(&hex[4..6], 16).unwrap_or(0);
                let _ = write!(out, "\x1b[38;2;{r};{g};{b}m");
            } else if tag == "</span>" {
                out.push_str("\x1b[39m");
            }
            // `<br>` and any other unknown tag: drop. The row break after `<br>`
            // is already carried by the following '\n'.
            rest = &rest[end + 1..];
        } else {
            out.push_str(rest);
            break;
        }
    }
    out
}

/// Prepend and append `n` blank rows to the cfonts output. `n` newline chars
/// before the first glyph row (each is a row-break into a new empty row),
/// and `n` after the last glyph row.
fn pad_vertically(s: &str, n: u16) -> String {
    if n == 0 {
        return s.to_string();
    }
    let pad: String = "\n".repeat(n as usize);
    let mut out = String::with_capacity(s.len() + pad.len() * 2);
    out.push_str(&pad);
    out.push_str(s);
    out.push_str(&pad);
    out
}

/// Parse cfonts output into cells for the streaming pipeline.
pub fn render_cells(cfg: &RenderConfig) -> Result<Vec<Cell>, RenderError> {
    Ok(sgr::parse(&render_raw(cfg)?))
}

/// Apply a filter to a cell grid at frame N and emit the ANSI bytes.
pub fn emit_shaded<F: Filter>(cells: &[Cell], filter: &F, frame: u64) -> String {
    let mut out = String::with_capacity(cells.len() * 4);
    sgr::emit_with(cells, |c| filter.shade(c, frame), &mut out);
    out
}

pub fn render_config(cfg: &RenderConfig) -> Result<String, RenderError> {
    if matches!(cfg.mode, Some(Mode::Rainbow)) {
        let cells = render_cells(cfg)?;
        return Ok(emit_shaded(&cells, &Rainbow, 0));
    }
    render_raw(cfg)
}

pub fn banner() -> String {
    let cfg = RenderConfig {
        text: "SHOUT".into(),
        ..Default::default()
    };
    render_config(&cfg).unwrap_or_else(|_| String::from("SHOUT\n"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base(text: &str) -> RenderConfig {
        RenderConfig {
            text: text.into(),
            ..Default::default()
        }
    }

    #[test]
    fn empty_text_rejected() {
        assert_eq!(render_config(&base("")), Err(RenderError::EmptyText));
    }

    #[test]
    fn unknown_font_rejected() {
        let cfg = RenderConfig {
            font: "standard".into(),
            ..base("hi")
        };
        assert_eq!(render_config(&cfg), Err(RenderError::UnknownFont));
    }

    #[test]
    fn default_renders_non_empty() {
        assert!(!render_config(&base("hi")).unwrap().is_empty());
    }

    #[test]
    fn solid_red_emits_red_sgr() {
        let cfg = RenderConfig {
            color: "red".into(),
            ..base("hi")
        };
        assert!(render_config(&cfg).unwrap().contains("\x1b[31m"));
    }

    #[test]
    fn solid_mode_defaults_to_white() {
        let cfg = RenderConfig {
            mode: Some(Mode::Solid),
            ..base("hi")
        };
        assert!(render_config(&cfg).unwrap().contains("\x1b["));
    }

    #[test]
    fn fire_mode_emits_truecolor_sgr() {
        let cfg = RenderConfig {
            mode: Some(Mode::Fire),
            ..base("hi")
        };
        assert!(render_config(&cfg).unwrap().contains("\x1b[38;2;"));
    }

    #[test]
    fn rainbow_mode_emits_truecolor_sgr() {
        // Phase-2: rainbow uses the HSL shader pipeline, which always emits truecolor.
        let cfg = RenderConfig {
            mode: Some(Mode::Rainbow),
            ..base("hi")
        };
        assert!(render_config(&cfg).unwrap().contains("\x1b[38;2;"));
    }

    #[test]
    fn unknown_color_rejected() {
        let cfg = RenderConfig {
            color: "puce".into(),
            ..base("hi")
        };
        assert_eq!(render_config(&cfg), Err(RenderError::UnknownColor));
    }

    #[test]
    fn is_color_matches_supported() {
        assert!(is_color("red"));
        assert!(is_color("cyanbright"));
        assert!(!is_color("puce"));
    }

    #[test]
    fn preset_emits_truecolor() {
        let cfg = RenderConfig {
            preset: "sunset".into(),
            ..base("hi")
        };
        assert!(render_config(&cfg).unwrap().contains("\x1b[38;2;"));
    }

    #[test]
    fn preset_on_single_color_font_ok() {
        // tiny is a 1-color font; preset is 2-stop. Should render without panic.
        let cfg = RenderConfig {
            font: "tiny".into(),
            preset: "neon".into(),
            ..base("hi")
        };
        assert!(render_config(&cfg).unwrap().contains("\x1b[38;2;"));
    }

    #[test]
    fn unknown_preset_rejected() {
        let cfg = RenderConfig {
            preset: "puce".into(),
            ..base("hi")
        };
        assert_eq!(render_config(&cfg), Err(RenderError::UnknownPreset));
    }

    #[test]
    fn preset_wins_over_bare_color() {
        // Both set via path classifier; preset should drive the render.
        let cfg = RenderConfig {
            preset: "ocean".into(),
            color: "red".into(),
            ..base("hi")
        };
        // If bare color won we'd see `\x1b[31m`, not truecolor.
        let out = render_config(&cfg).unwrap();
        assert!(
            out.contains("\x1b[38;2;"),
            "expected truecolor (preset), got: {out}"
        );
        assert!(!out.contains("\x1b[31m"));
    }

    #[test]
    fn preset_on_multi_slot_font_uses_two_distinct_colors() {
        // 3d is a 2-slot font; neon has two very different stops. We expect
        // both stops to appear as solid SGR runs (cfonts `-c A,B` behavior),
        // not blended into intermediate gradient shades.
        let cfg = RenderConfig {
            font: "3d".into(),
            preset: "neon".into(),
            ..base("hi")
        };
        let out = render_config(&cfg).unwrap();
        // neon stops: #ff00ea and #00eaff
        assert!(
            out.contains("\x1b[38;2;255;0;234m"),
            "expected slot-1 color in output"
        );
        assert!(
            out.contains("\x1b[38;2;0;234;255m"),
            "expected slot-2 color in output"
        );
    }

    #[test]
    fn rainbow_on_multi_slot_font_tags_slots() {
        // Cells from a 2-slot font under rainbow should arrive tagged with
        // both slot sentinels so the shader can differentiate them.
        let cfg = RenderConfig {
            font: "3d".into(),
            mode: Some(Mode::Rainbow),
            ..base("hi")
        };
        let cells = render_cells(&cfg).unwrap();
        let has_s0 = cells.iter().any(|c| c.rgb == Some(SLOT_SENTINELS[0]));
        let has_s1 = cells.iter().any(|c| c.rgb == Some(SLOT_SENTINELS[1]));
        assert!(has_s0 && has_s1, "expected both slot sentinels in cells");
    }

    #[test]
    fn fire_on_multi_slot_font_tags_slots() {
        let cfg = RenderConfig {
            font: "3d".into(),
            mode: Some(Mode::Fire),
            ..base("hi")
        };
        let cells = render_cells(&cfg).unwrap();
        let has_s0 = cells.iter().any(|c| c.rgb == Some(SLOT_SENTINELS[0]));
        let has_s1 = cells.iter().any(|c| c.rgb == Some(SLOT_SENTINELS[1]));
        assert!(has_s0 && has_s1, "expected both slot sentinels in cells");
    }

    #[test]
    fn browser_to_sgr_strips_wrapper_and_rewrites_spans() {
        let in_ = "<div style=\"font-family:monospace\"><span style=\"color:#ff00aa\">A█</span><br>\n<span style=\"color:#00aaff\">B</span></div>";
        let got = browser_to_sgr(in_);
        assert_eq!(
            got,
            "\x1b[38;2;255;0;170mA█\x1b[39m\n\x1b[38;2;0;170;255mB\x1b[39m"
        );
    }

    #[test]
    fn browser_no_wrap_at_80_cols() {
        // Long input that would wrap at terminal_width=80 in Env::Cli. In
        // browser mode the wrap ceiling is 0xFFFF, so output stays one line.
        let cfg = RenderConfig {
            text: "ABCDEFGHIJ".into(),
            font: "block".into(),
            browser: true,
            padding: 0,
            ..Default::default()
        };
        let out = render_config(&cfg).unwrap();
        // block font is 6 rows tall — one unwrapped banner should produce 5
        // newlines (between rows). A wrap would double that.
        let lines = out.matches('\n').count();
        assert!(
            lines <= 6,
            "expected <=6 line breaks (one banner), got {lines} in {out:?}"
        );
        // The Env::Browser wrapper div must have been scrubbed.
        assert!(!out.contains("<div"), "div wrapper leaked: {out:?}");
        assert!(!out.contains("<br>"), "<br> leaked: {out:?}");
    }

    #[test]
    fn render_cells_returns_non_empty() {
        let cfg = RenderConfig {
            mode: Some(Mode::Fire),
            ..base("hi")
        };
        let cells = render_cells(&cfg).unwrap();
        assert!(!cells.is_empty());
        assert!(cells.iter().any(|c| c.rgb.is_some()));
    }
}
