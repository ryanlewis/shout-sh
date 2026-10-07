// shout.sh — curl-friendly ANSI banner service
// Copyright (C) 2026 Ryan Lewis
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

use std::num::NonZeroUsize;

use cfonts::{
    Background, BackgroundOption, BlockOptions, CliEnv, Color, ColorLevel, ColorOption,
    ColorOverride, Gradient, GradientOption, Kind, Options, RenderOverrides, Rgb, Text,
    TransitionStops, render_with,
};

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

fn color_enum<K: Kind>(name: &str) -> Option<Color<K>> {
    Some(match name {
        "red" => Color::RED,
        "green" => Color::GREEN,
        "blue" => Color::BLUE,
        "yellow" => Color::YELLOW,
        "cyan" => Color::CYAN,
        "magenta" => Color::MAGENTA,
        "white" => Color::WHITE,
        "gray" => Color::GRAY,
        "redbright" => Color::RED_BRIGHT,
        "greenbright" => Color::GREEN_BRIGHT,
        "bluebright" => Color::BLUE_BRIGHT,
        "yellowbright" => Color::YELLOW_BRIGHT,
        "cyanbright" => Color::CYAN_BRIGHT,
        "magentabright" => Color::MAGENTA_BRIGHT,
        "whitebright" => Color::WHITE_BRIGHT,
        _ => return None,
    })
}

pub fn is_color(name: &str) -> bool {
    color_enum::<Text>(name).is_some()
}

/// `None` is no background: cfonts leaves the terminal's own.
fn bg_color_enum(name: &str) -> Option<Option<Color<Background>>> {
    match name {
        "" | "transparent" => Some(None),
        "black" => Some(Some(Color::BLACK)),
        _ => color_enum(name).map(Some),
    }
}

pub fn is_bg_color(name: &str) -> bool {
    bg_color_enum(name).is_some()
}

/// For shader-driven modes: paint each of the font's slots with a distinct
/// sentinel RGB so the shader can see which slot a cell came from. For
/// single-slot fonts there's nothing to differentiate, so `fallback` is
/// used instead (typically `Color::WHITE` for rainbow).
fn slot_sentinels(slots: usize, fallback: Color) -> ColorOption {
    if slots >= 2 {
        ColorOption::Colors(
            (0..slots)
                .map(|i| {
                    let (red, green, blue) = SLOT_SENTINELS[i.min(SLOT_SENTINELS.len() - 1)];
                    Color::rgb(red, green, blue)
                })
                .collect(),
        )
    } else {
        ColorOption::Colors(vec![fallback])
    }
}

/// Stops of the Fire gradient on single-slot fonts. Hex, not cfonts'
/// named colors: cfonts' red is #ea3223.
const FIRE_STOPS: [&str; 3] = ["#ff0000", "#ff9900", "#ffff00"];

/// Parse a built-in hex stop. Every preset and Fire stop is checked by a
/// test, so a failure here is a typo in this crate.
fn hex_to_rgb(hex: &str) -> Rgb {
    Rgb::from_hex(hex).expect("built-in stops are #rrggbb")
}

/// A transition gradient through `stops`, which must hold at least one.
/// A single stop is repeated, as a transition needs two.
fn transition(stops: &[&str]) -> ColorOption {
    let mut stops: Vec<Color<Gradient>> =
        stops.iter().map(|s| Color::from(hex_to_rgb(s))).collect();
    if stops.len() < 2 {
        stops.push(stops[0]);
    }
    let stops = TransitionStops::try_from(stops).expect("at least two stops");
    ColorOption::Gradient(GradientOption::Transition(stops))
}

/// Map a preset onto cfonts. Multi-slot fonts (3d, block, chrome, ...) get
/// one solid color per slot — matches `cfonts -c A,B` and keeps slot 1 and
/// slot 2 visually distinct. Single-slot fonts fall back to the transition
/// gradient so the palette still reads across the text.
fn apply_preset(slots: usize, preset_name: &str) -> Result<ColorOption, RenderError> {
    let preset = presets::resolve(preset_name).ok_or(RenderError::UnknownPreset)?;
    if slots >= 2 {
        let mut stops: Vec<&str> = preset.stops.iter().copied().take(slots).collect();
        while stops.len() < slots {
            stops.push(*stops.last().unwrap());
        }
        Ok(ColorOption::Colors(
            stops
                .into_iter()
                .map(|s| Color::from(hex_to_rgb(s)))
                .collect(),
        ))
    } else {
        Ok(transition(preset.stops))
    }
}

/// The error `render_config` would return for `cfg`, found without
/// rendering. `render_raw` runs this first, so a caller that must decide
/// before rendering (the Worker's stream slot) agrees with the render.
pub fn check(cfg: &RenderConfig) -> Result<(), RenderError> {
    if cfg.text.is_empty() {
        return Err(RenderError::EmptyText);
    }
    if !fonts::is_font(&cfg.font) {
        return Err(RenderError::UnknownFont);
    }
    match cfg.mode {
        // A preset wins over a color. Solid with neither is white.
        None | Some(Mode::Solid) if !cfg.preset.is_empty() => {
            presets::resolve(&cfg.preset).ok_or(RenderError::UnknownPreset)?;
        }
        None | Some(Mode::Solid) if !cfg.color.is_empty() => {
            color_enum::<Text>(&cfg.color).ok_or(RenderError::UnknownColor)?;
        }
        // The shaders pick the colors; a color or preset is ignored.
        None | Some(Mode::Solid | Mode::Rainbow | Mode::Fire) => {}
    }
    Ok(())
}

/// Columns a CLI render wraps at. cfonts 1.3.0 wrapped at the terminal
/// width and fell back to 80 without a terminal, as in the Worker; v4 takes
/// the width from the caller, so the native build now matches the Worker.
const CLI_WIDTH: usize = 80;

/// Columns a browser render wraps at: 1.3.0's `Env::Browser` ceiling. It
/// keeps every column a `u16` for the cell grid.
const BROWSER_WIDTH: usize = u16::MAX as usize;

/// Raw cfonts render as String. For Rainbow mode, renders with a neutral
/// white base so the shader pipeline can recolor every non-space glyph.
fn render_raw(cfg: &RenderConfig) -> Result<String, RenderError> {
    check(cfg)?;

    let font = resolve(&cfg.font).ok_or(RenderError::UnknownFont)?;

    let slots = font.get_font().colors();

    let colors = match cfg.mode {
        // A preset wins over a color. Solid with neither is white.
        None | Some(Mode::Solid) if !cfg.preset.is_empty() => {
            Some(apply_preset(slots, &cfg.preset)?)
        }
        None | Some(Mode::Solid) if !cfg.color.is_empty() => Some(ColorOption::Colors(vec![
            color_enum(&cfg.color).ok_or(RenderError::UnknownColor)?,
        ])),
        None => None,
        Some(Mode::Solid) => Some(ColorOption::Colors(vec![Color::WHITE])),
        // Multi-slot fonts get per-slot sentinel colors so the shader can
        // differentiate front/shadow layers. Single-slot fonts fall back
        // to a neutral white base (shader recolors every non-space cell).
        // Borders emit bare and are picked up via `char != ' '`.
        Some(Mode::Rainbow) => Some(slot_sentinels(slots, Color::WHITE)),
        // Same pattern as rainbow — multi-slot fonts get sentinels, so the
        // Fire shader can render slot 1+ as dim embers behind the flame.
        // Single-slot fonts use the legacy gradient path.
        Some(Mode::Fire) if slots >= 2 => Some(slot_sentinels(slots, Color::WHITE)),
        Some(Mode::Fire) => Some(transition(&FIRE_STOPS)),
    };

    let mut block = BlockOptions::new(cfg.text.as_str());
    block.font = font;
    block.letter_spacing = usize::from(cfg.letter_spacing);
    // One blank row between lines. Every font but console declares this;
    // v4's console declares none, where 1.3.0 left one.
    block.line_height = Some(1);
    // The browser has no terminal palette: paint named colors with the RGB
    // values cfonts' browser output uses, as 1.3.0's `Env::Browser` did.
    block.colors = if cfg.browser {
        colors.map(flatten_named)
    } else {
        colors
    };
    // The playground paints the background itself, so only the CLI
    // output carries it.
    let background = if cfg.browser {
        None
    } else {
        bg_color_enum(&cfg.background).flatten()
    };

    // cfonts' own `spaceless=false` hardcodes 2 blank rows above + below.
    // Always pass `spaceless=true` so we can add `cfg.padding` rows ourselves
    // — gives the caller a continuous 0..=MAX_PADDING knob.
    let opts = Options {
        spaceless: true,
        max_length: NonZeroUsize::new(usize::from(cfg.max_length)),
        background: background.map(BackgroundOption::Color),
        blocks: vec![block],
        ..Options::default()
    };

    let width = if cfg.browser {
        BROWSER_WIDTH
    } else {
        CLI_WIDTH
    };
    let overrides = RenderOverrides::default()
        .with_canvas_width(width)
        .with_color(ColorOverride::Level(ColorLevel::TrueColor));
    let raw = render_with(&opts, &CliEnv::default(), overrides).text;
    // Belt-and-braces against a future cfonts regression emitting anything
    // beyond plain SGR.
    Ok(pad_vertically(&sanitize_ansi(&raw), cfg.padding))
}

/// Swap each named color in `colors` for its RGB value.
fn flatten_named(colors: ColorOption) -> ColorOption {
    match colors {
        ColorOption::Colors(colors) => ColorOption::Colors(
            colors
                .into_iter()
                .map(|c| c.to_rgb().map_or(c, Color::from))
                .collect(),
        ),
        gradient => gradient,
    }
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
    let mut out = String::with_capacity(sgr::emit_capacity(cells));
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
    fn browser_render_keeps_glyph_angle_brackets() {
        // `simple` draws these letters with '<'. The CLI render wraps at 80
        // columns, so render one letter at a time to keep it on one line.
        let font = "simple";
        for letter in "$kx".chars() {
            for (mode, preset) in [
                (Some(Mode::Fire), ""),
                (Some(Mode::Rainbow), ""),
                (Some(Mode::Solid), "sunset"),
                (None, ""),
            ] {
                let cfg = RenderConfig {
                    text: letter.into(),
                    font: font.into(),
                    mode,
                    preset: preset.into(),
                    padding: 0,
                    ..Default::default()
                };
                let cli = render_cells(&cfg).unwrap();
                let browser = render_cells(&RenderConfig {
                    browser: true,
                    ..cfg.clone()
                })
                .unwrap();
                assert!(cli.iter().any(|c| c.ch == '<'), "{font} {letter}: no '<'");
                assert_eq!(
                    layout(&browser),
                    layout(&cli),
                    "{font} {letter} {mode:?} {preset}"
                );
            }
        }
    }

    #[test]
    fn browser_no_wrap_at_80_cols() {
        // Long input that would wrap at 80 columns in a CLI render. In
        // browser mode the canvas is unlimited, so output stays one line.
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
    }

    fn browser_cfg(text: &str, font: &str, mode: Mode, preset: &str) -> RenderConfig {
        RenderConfig {
            text: text.into(),
            font: font.into(),
            mode: Some(mode),
            preset: preset.into(),
            browser: true,
            padding: 0,
            ..Default::default()
        }
    }

    fn layout(cells: &[Cell]) -> Vec<(u16, u16, char)> {
        cells.iter().map(|c| (c.row, c.col, c.ch)).collect()
    }

    #[test]
    fn browser_gradient_wider_than_i8_paints_end_to_end() {
        // One line of this pangram is over 255 columns. cfonts 1.3.0 counted
        // gradient steps in i8: a gap of 127 columns painted a flat run of
        // the first stop, a longer one panicked (a trap in the wasm
        // playground). Two stops have one gap and Fire's three have two, so
        // each line is wider than 1.3.0 could paint. Both must ramp from the
        // first stop to the last across the whole line.
        let text = "the quick brown fox jumps over the lazy dog";
        for (cfg, min_cols, first, last) in [
            (
                browser_cfg(text, "simple", Mode::Fire, ""),
                2 * 127 + 3,
                (255, 0, 0),
                (255, 255, 0),
            ),
            (
                browser_cfg(text, "tiny", Mode::Solid, "sunset"),
                127 + 2,
                (255, 179, 71),
                (255, 51, 102),
            ),
        ] {
            let cells = render_cells(&cfg).unwrap();
            let one_line = render_cells(&browser_cfg("t", &cfg.font, Mode::Rainbow, "")).unwrap();
            assert_eq!(sgr::row_count(&cells), sgr::row_count(&one_line));
            let ink: Vec<&Cell> = cells
                .iter()
                .filter(|c| c.ch != ' ' && c.rgb.is_some())
                .collect();
            let left = ink.iter().min_by_key(|c| c.col).unwrap();
            let right = ink.iter().max_by_key(|c| c.col).unwrap();
            assert!(right.col >= min_cols, "{}: {} columns", cfg.font, right.col);
            let near = |a: sgr::Rgb, b: sgr::Rgb| {
                a.0.abs_diff(b.0) <= 8 && a.1.abs_diff(b.1) <= 8 && a.2.abs_diff(b.2) <= 8
            };
            assert!(
                near(left.rgb.unwrap(), first),
                "{}: {:?}",
                cfg.font,
                left.rgb
            );
            assert!(
                near(right.rgb.unwrap(), last),
                "{}: {:?}",
                cfg.font,
                right.rgb
            );
            let distinct: std::collections::BTreeSet<_> = ink.iter().map(|c| c.rgb).collect();
            assert!(
                distinct.len() > 64,
                "{}: {} colors",
                cfg.font,
                distinct.len()
            );
        }
    }

    #[test]
    fn browser_wraps_at_the_u16_column_limit() {
        // Unwrapped, this line is about 80,000 columns: past what a cell's
        // u16 column holds, which overflows in a debug build.
        let cfg = browser_cfg(&"W".repeat(8_000), "block", Mode::Solid, "");
        let cells = render_cells(&cfg).unwrap();
        let cols = cells.iter().map(|c| u32::from(c.col) + 1).max().unwrap();
        assert!(cols <= u32::from(u16::MAX), "{cols} columns");
        let one_line = render_cells(&browser_cfg("W", "block", Mode::Solid, "")).unwrap();
        assert!(sgr::row_count(&cells) > sgr::row_count(&one_line));
    }

    #[test]
    fn console_leaves_a_blank_row_between_lines() {
        let cfg = RenderConfig {
            font: "console".into(),
            padding: 0,
            ..base("ab|cd")
        };
        assert_eq!(render_config(&cfg).unwrap(), "ab\n\ncd");
    }

    #[test]
    fn browser_render_has_no_background() {
        let cfg = RenderConfig {
            background: "blue".into(),
            ..browser_cfg("hi", "block", Mode::Solid, "")
        };
        assert!(!render_config(&cfg).unwrap().contains("\x1b[44m"));
        let cli = RenderConfig {
            browser: false,
            ..cfg
        };
        assert!(render_config(&cli).unwrap().contains("\x1b[44m"));
    }

    #[test]
    fn built_in_stops_parse() {
        for stop in presets::PRESETS
            .iter()
            .flat_map(|p| p.stops)
            .chain(&FIRE_STOPS)
        {
            assert!(Rgb::from_hex(stop).is_ok(), "{stop}");
        }
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

    fn check_capacity<F: Filter>(cells: &[Cell], filter: &F) {
        for frame in 0..8 {
            let out = emit_shaded(cells, filter, frame);
            assert!(out.len() <= sgr::emit_capacity(cells), "frame {frame}");
        }
    }

    #[test]
    fn emit_shaded_fits_capacity() {
        use crate::shader::{Identity, Shader};
        for mode in [Mode::Solid, Mode::Rainbow, Mode::Fire] {
            let cells = render_cells(&RenderConfig {
                mode: Some(mode),
                ..base("shout.sh")
            })
            .unwrap();
            let rows = sgr::row_count(&cells);
            check_capacity(&cells, &Shader::for_mode(mode, rows));
        }
        // Worst case: every cell opens a new three-digit colour and is a
        // 4-byte char.
        let worst: Vec<Cell> = (0..40u16)
            .map(|i| Cell {
                ch: '\u{1D11E}',
                rgb: Some((255 - (i % 2) as u8, 200, 100)),
                row: i / 10,
                col: i % 10,
            })
            .collect();
        check_capacity(&worst, &Identity);
        check_capacity(&[], &Rainbow);
    }
}
