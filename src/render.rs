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

use cfonts::{Colors, Options, render};

use crate::fonts::resolve;
use crate::parser::{Mode, RenderConfig};

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

pub fn render_config(cfg: &RenderConfig) -> Result<String, RenderError> {
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
            opts.colors = vec![Colors::Candy];
        }
        Some(Mode::Fire) => {
            // Named-color gradients panic cfonts (see spike-findings); always pass hex.
            opts.gradient = vec!["#ff0000".into(), "#ff9900".into(), "#ffff00".into()];
            opts.transition_gradient = true;
        }
    }

    Ok(render(opts).text)
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
    fn rainbow_mode_renders() {
        let cfg = RenderConfig {
            mode: Some(Mode::Rainbow),
            ..base("hi")
        };
        assert!(!render_config(&cfg).unwrap().is_empty());
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
}
