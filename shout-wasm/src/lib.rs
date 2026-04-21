// shout.sh — curl-friendly ANSI banner service
// Copyright (C) 2026 Ryan Lewis
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

//! wasm-bindgen shim that ships the shout-core render pipeline to the
//! browser. Two exports: `render_once_html` for static frames and
//! `render_frame_html(cfg, frame)` for animated modes driven by the JS
//! requestAnimationFrame loop.

use serde::Deserialize;
use shout_core::emit_html::emit_html_body;
use shout_core::parser::{Mode, RenderConfig};
use shout_core::render::{render_cells, render_config as render_ansi};
use shout_core::sgr::{self, Cell};
use shout_core::shader::{Filter, Fire, Identity, Rainbow};
use wasm_bindgen::prelude::*;

/// JSON shape accepted over the wasm boundary. Fields mirror the server-side
/// `RenderConfig` subset the playground actually uses.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "lowercase")]
struct JsCfg {
    text: String,
    font: String,
    mode: Option<String>,
    color: String,
    preset: String,
}

fn cfg_from_json(s: &str) -> Result<RenderConfig, String> {
    let raw: JsCfg = serde_json::from_str(s).map_err(|e| e.to_string())?;
    let mode = match raw.mode.as_deref() {
        None | Some("") | Some("none") => None,
        Some(m) => match Mode::from_token(m) {
            Some(parsed) => Some(parsed),
            None => return Err(format!("unknown mode: {m}")),
        },
    };
    let font = if raw.font.is_empty() {
        "block".to_string()
    } else {
        raw.font
    };
    Ok(RenderConfig {
        text: raw.text,
        font,
        mode,
        color: raw.color,
        preset: raw.preset,
        ..Default::default()
    })
}

fn render_once_inner(cfg_json: &str) -> Result<String, String> {
    let cfg = cfg_from_json(cfg_json)?;
    match cfg.mode {
        Some(Mode::Rainbow) | Some(Mode::Fire) => {
            let cells = render_cells(&cfg).map_err(|e| e.message().to_string())?;
            Ok(render_frame(&cells, cfg.mode.unwrap(), 0))
        }
        _ => {
            let _ansi = render_ansi(&cfg).map_err(|e| e.message().to_string())?;
            let cells = render_cells(&cfg).map_err(|e| e.message().to_string())?;
            Ok(render_frame_identity(&cells))
        }
    }
}

fn render_frame_inner(cfg_json: &str, frame: u32) -> Result<String, String> {
    let cfg = cfg_from_json(cfg_json)?;
    let cells = render_cells(&cfg).map_err(|e| e.message().to_string())?;
    let mode = cfg.mode.unwrap_or(Mode::Solid);
    Ok(render_frame(&cells, mode, frame as u64))
}

/// Render a single static frame as the inner `<pre>`-body HTML.
///
/// For solid/color modes this returns the identity shader's output. For
/// rainbow/fire it renders frame 0 as a stable snapshot — use
/// `render_frame_html` to animate.
#[wasm_bindgen]
pub fn render_once_html(cfg_json: &str) -> Result<String, JsError> {
    render_once_inner(cfg_json).map_err(|e| JsError::new(&e))
}

/// Render frame N of an animated banner as the inner `<pre>`-body HTML.
/// Caller drives a requestAnimationFrame loop and increments `frame`.
#[wasm_bindgen]
pub fn render_frame_html(cfg_json: &str, frame: u32) -> Result<String, JsError> {
    render_frame_inner(cfg_json, frame).map_err(|e| JsError::new(&e))
}

fn render_frame(cells: &[Cell], mode: Mode, frame: u64) -> String {
    let cells = trim_blank_rows(cells);
    let mut out = String::with_capacity(cells.len() * 16);
    match mode {
        Mode::Rainbow => emit_html_body(&cells, |c| Rainbow.shade(c, frame), &mut out),
        Mode::Fire => {
            let rows = sgr::row_count(&cells);
            let fire = Fire { rows };
            emit_html_body(&cells, |c| fire.shade(c, frame), &mut out);
        }
        Mode::Solid => emit_html_body(&cells, |c| Identity.shade(c, frame), &mut out),
    }
    out
}

fn render_frame_identity(cells: &[Cell]) -> String {
    let cells = trim_blank_rows(cells);
    let mut out = String::with_capacity(cells.len() * 16);
    emit_html_body(&cells, |c| Identity.shade(c, 0), &mut out);
    out
}

/// Drop all-whitespace rows and renumber the survivors — cfonts pads its
/// output with blank rows that look fine in a terminal (they're just more
/// of the scrollback) but leave ugly empty lines in a bordered HTML frame.
fn trim_blank_rows(cells: &[Cell]) -> Vec<Cell> {
    let total_rows = sgr::row_count(cells);
    if total_rows == 0 {
        return Vec::new();
    }
    let mut nonblank = vec![false; total_rows as usize];
    for c in cells {
        if !c.ch.is_whitespace() {
            nonblank[c.row as usize] = true;
        }
    }
    let mut row_map = vec![u16::MAX; total_rows as usize];
    let mut next = 0u16;
    for (old, keep) in nonblank.iter().enumerate() {
        if *keep {
            row_map[old] = next;
            next += 1;
        }
    }
    cells
        .iter()
        .filter(|c| nonblank[c.row as usize])
        .map(|c| Cell {
            ch: c.ch,
            rgb: c.rgb,
            row: row_map[c.row as usize],
            col: c.col,
        })
        .collect()
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use super::*;

    #[test]
    fn rainbow_frames_differ() {
        let cfg = r#"{"text":"HI","font":"block","mode":"rainbow"}"#;
        let f0 = render_frame_inner(cfg, 0).unwrap();
        let f30 = render_frame_inner(cfg, 30).unwrap();
        assert_ne!(f0, f30);
        assert!(f0.contains("<span"));
    }

    #[test]
    fn solid_red_has_red_span() {
        let cfg = r#"{"text":"HI","font":"tiny","color":"red"}"#;
        let out = render_once_inner(cfg).unwrap();
        assert!(out.contains("color:#"), "got: {out}");
    }

    #[test]
    fn xss_text_escaped() {
        // cfonts won't glyph '<' but if it ever did (or a future font path
        // passed text through), the emitter must escape it. Use a bare cell
        // path via font=block; the explicit check is the unit test in core.
        let cfg = r#"{"text":"<script>","font":"tiny"}"#;
        let out = render_once_inner(cfg).unwrap();
        assert!(
            !out.contains("<script"),
            "raw <script reached wasm output: {out}"
        );
    }

    #[test]
    fn unknown_mode_errors() {
        let cfg = r#"{"text":"HI","mode":"matrix"}"#;
        assert!(render_once_inner(cfg).is_err());
    }

    #[test]
    fn empty_text_errors() {
        let cfg = r#"{"text":""}"#;
        assert!(render_once_inner(cfg).is_err());
    }

    #[test]
    fn default_font_when_missing() {
        let cfg = r#"{"text":"HI"}"#;
        let out = render_once_inner(cfg).unwrap();
        assert!(!out.is_empty());
    }

    #[test]
    fn preset_renders_truecolor_spans() {
        let cfg = r#"{"text":"HI","font":"block","preset":"sunset"}"#;
        let out = render_once_inner(cfg).unwrap();
        assert!(out.contains("color:#"), "no colored spans: {out}");
    }
}
