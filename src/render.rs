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
use crate::parser::RenderConfig;

#[derive(Debug, PartialEq, Eq)]
pub enum RenderError {
    UnknownFont,
    UnknownMode,
    UnknownColor,
    EmptyText,
}

impl RenderError {
    pub fn message(&self) -> &'static str {
        // Errors follow the design system: lowercase, period-terminated,
        // with a fix hinted in backticks.
        match self {
            Self::UnknownFont => "font not found. try `curl shout.sh/fonts`.",
            Self::UnknownMode => "mode not found. try `solid`, `rainbow`, or `fire`.",
            Self::UnknownColor => {
                "color not found. try `red`, `green`, `blue`, `yellow`, `cyan`, `magenta`, `white`, `gray`, or a `*bright` variant."
            }
            Self::EmptyText => "nothing to shout about. type something.",
        }
    }
}

/// Named color → cfonts Colors enum. Mapping is total over the color list
/// exposed by the parser.
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

    match cfg.mode.as_str() {
        "" => {
            if !cfg.color.is_empty() {
                let c = color_enum(&cfg.color).ok_or(RenderError::UnknownColor)?;
                opts.colors = vec![c];
            }
        }
        "solid" => {
            let name = if cfg.color.is_empty() {
                "white"
            } else {
                cfg.color.as_str()
            };
            let c = color_enum(name).ok_or(RenderError::UnknownColor)?;
            opts.colors = vec![c];
        }
        "rainbow" => {
            // Candy = per-char random bright palette; closest static analog
            // to legacy's animated rainbow shader.
            opts.colors = vec![Colors::Candy];
        }
        "fire" => {
            // Warm 3-stop hex gradient. Named-color gradients panic cfonts
            // (see spike-findings); always pass hex.
            opts.gradient = vec!["#ff0000".into(), "#ff9900".into(), "#ffff00".into()];
            opts.transition_gradient = true;
        }
        _ => return Err(RenderError::UnknownMode),
    }

    let out = render(opts);
    Ok(out.text)
}

/// Render the SHOUT banner for the help page. Panic-free: if something
/// goes sideways at startup we'd rather know immediately than serve
/// garbage — but the inputs here are compile-time constants so the
/// unwrap path is unreachable in practice.
pub fn banner() -> String {
    let cfg = RenderConfig {
        text: "SHOUT".into(),
        font: "block".into(),
        mode: String::new(),
        color: String::new(),
        format: String::new(),
    };
    render_config(&cfg).unwrap_or_else(|_| String::from("SHOUT\n"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base(text: &str) -> RenderConfig {
        RenderConfig {
            text: text.into(),
            font: "block".into(),
            mode: String::new(),
            color: String::new(),
            format: String::new(),
        }
    }

    #[test]
    fn empty_text_rejected() {
        let cfg = base("");
        assert_eq!(render_config(&cfg), Err(RenderError::EmptyText));
    }

    #[test]
    fn unknown_font_rejected() {
        let mut cfg = base("hi");
        cfg.font = "standard".into();
        assert_eq!(render_config(&cfg), Err(RenderError::UnknownFont));
    }

    #[test]
    fn default_renders_non_empty() {
        let out = render_config(&base("hi")).unwrap();
        assert!(!out.is_empty());
    }

    #[test]
    fn solid_red_emits_red_sgr() {
        let mut cfg = base("hi");
        cfg.color = "red".into();
        let out = render_config(&cfg).unwrap();
        assert!(out.contains("\x1b[31m"), "expected red SGR in output");
    }

    #[test]
    fn solid_mode_defaults_to_white() {
        let mut cfg = base("hi");
        cfg.mode = "solid".into();
        let out = render_config(&cfg).unwrap();
        assert!(out.contains("\x1b["), "expected ANSI SGR in output");
    }

    #[test]
    fn fire_mode_emits_truecolor_sgr() {
        let mut cfg = base("hi");
        cfg.mode = "fire".into();
        let out = render_config(&cfg).unwrap();
        assert!(
            out.contains("\x1b[38;2;"),
            "expected truecolor SGR in fire output"
        );
    }

    #[test]
    fn rainbow_mode_renders() {
        let mut cfg = base("hi");
        cfg.mode = "rainbow".into();
        let out = render_config(&cfg).unwrap();
        assert!(!out.is_empty());
    }

    #[test]
    fn unknown_mode_rejected() {
        let mut cfg = base("hi");
        cfg.mode = "matrix".into();
        assert_eq!(render_config(&cfg), Err(RenderError::UnknownMode));
    }
}
