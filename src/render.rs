// shout.sh — curl-friendly ANSI banner service
// Copyright (C) 2026 Ryan Lewis
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

use cfonts::{Colors, Options, render};

use crate::fonts::resolve;
use crate::parser::{Mode, RenderConfig};
use crate::sgr::{self, Cell};
use crate::shader::{Filter, Rainbow};

#[derive(Debug, PartialEq, Eq)]
pub enum RenderError {
    UnknownFont,
    UnknownColor,
    EmptyText,
}

impl RenderError {
    pub fn message(&self) -> &'static str {
        match self {
            Self::UnknownFont => "font not found. try `curl shout.sh/fonts`.",
            Self::UnknownColor => {
                "color not found. try `red`, `green`, `blue`, `yellow`, `cyan`, `magenta`, `white`, `gray`, or a `*bright` variant."
            }
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

/// Raw cfonts render as String. For Rainbow mode, renders with a neutral
/// white base so the shader pipeline can recolor every non-space glyph.
fn render_raw(cfg: &RenderConfig) -> Result<String, RenderError> {
    if cfg.text.is_empty() {
        return Err(RenderError::EmptyText);
    }

    let font = resolve(&cfg.font).ok_or(RenderError::UnknownFont)?;

    let mut opts = Options {
        text: cfg.text.clone(),
        font,
        ..Options::default()
    };

    match cfg.mode {
        None => {
            if !cfg.color.is_empty() {
                opts.colors = vec![color_enum(&cfg.color).ok_or(RenderError::UnknownColor)?];
            }
        }
        Some(Mode::Solid) => {
            let name = if cfg.color.is_empty() {
                "white"
            } else {
                cfg.color.as_str()
            };
            opts.colors = vec![color_enum(name).ok_or(RenderError::UnknownColor)?];
        }
        Some(Mode::Rainbow) => {
            // Neutral base; the Rainbow shader recolors per-frame (frame 0
            // for static output). Borders emit bare and the shader picks
            // them up via char != ' '.
            opts.colors = vec![Colors::White];
        }
        Some(Mode::Fire) => {
            // Named-color gradients panic cfonts (see spike-findings); always pass hex.
            opts.gradient = vec!["#ff0000".into(), "#ff9900".into(), "#ffff00".into()];
            opts.transition_gradient = true;
        }
    }

    Ok(render(opts).text)
}

/// Parse cfonts output into cells for the streaming pipeline.
pub fn render_cells(cfg: &RenderConfig) -> Result<Vec<Cell>, RenderError> {
    Ok(sgr::parse(&render_raw(cfg)?))
}

/// Apply a filter to a cell grid at frame N and emit the ANSI bytes.
pub fn emit_shaded<F: Filter>(cells: &[Cell], filter: &F, frame: u64) -> String {
    let shaded: Vec<Cell> = cells
        .iter()
        .map(|c| Cell {
            ch: c.ch,
            row: c.row,
            col: c.col,
            rgb: filter.shade(c, frame),
        })
        .collect();
    sgr::emit(&shaded)
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
