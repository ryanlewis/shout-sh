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

use std::cell::RefCell;

use serde::Deserialize;
use shout_core::emit_html::{self, emit_html_body};
use shout_core::parser::{
    DEFAULT_LETTER_SPACING, DEFAULT_PADDING, MAX_LETTER_SPACING, MAX_MAX_LENGTH, MAX_PADDING, Mode,
    RenderConfig,
};
use shout_core::render::{render_cells, render_config as render_ansi};
use shout_core::sgr::{self, Cell};
use shout_core::shader::{Filter, Identity, Shader};
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
    #[serde(alias = "letterSpacing")]
    letter_spacing: Option<u16>,
    #[serde(alias = "maxLength")]
    max_length: Option<u16>,
    padding: Option<u16>,
    background: String,
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
        letter_spacing: raw
            .letter_spacing
            .map(|n| n.min(MAX_LETTER_SPACING))
            .unwrap_or(DEFAULT_LETTER_SPACING),
        max_length: raw.max_length.map(|n| n.min(MAX_MAX_LENGTH)).unwrap_or(0),
        padding: raw
            .padding
            .map(|n| n.min(MAX_PADDING))
            .unwrap_or(DEFAULT_PADDING),
        background: raw.background,
        browser: true,
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

/// The cell grid for the last config `render_frame_html` saw, keyed on its
/// JSON string. The playground sends the same JSON every frame, so this skips
/// cfonts and `sgr::parse` on all but the first frame of a config.
struct FrameCache {
    cfg_json: String,
    mode: Mode,
    cells: Vec<Cell>,
}

thread_local! {
    static FRAME_CACHE: RefCell<Option<FrameCache>> = const { RefCell::new(None) };
}

fn render_frame_inner(cfg_json: &str, frame: u32) -> Result<String, String> {
    // Take the cache out rather than holding a RefCell borrow across cfonts
    // and the shader. The release profile is panic = "abort", so a panic traps
    // without dropping guards: a borrow held at that point would stay set and
    // every later frame would fail with "already borrowed". Taken out, a trap
    // only loses the cache, and the next call rebuilds it.
    let c = match FRAME_CACHE.take() {
        Some(c) if c.cfg_json == cfg_json => c,
        _ => {
            let cfg = cfg_from_json(cfg_json)?;
            let mut cells = render_cells(&cfg).map_err(|e| e.message().to_string())?;
            // sgr::parse sizes the Vec from the raw ANSI length, several times
            // the cell count. Drop the slack before holding it across frames.
            cells.shrink_to_fit();
            FrameCache {
                cfg_json: cfg_json.to_string(),
                mode: cfg.mode.unwrap_or(Mode::Solid),
                cells,
            }
        }
    };
    let out = render_frame(&c.cells, c.mode, frame as u64);
    FRAME_CACHE.set(Some(c));
    Ok(out)
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
    let mut out = String::with_capacity(emit_html::emit_capacity(cells));
    let shader = Shader::for_mode(mode, sgr::row_count(cells));
    emit_html_body(cells, |c| shader.shade(c, frame), &mut out);
    out
}

fn render_frame_identity(cells: &[Cell]) -> String {
    let mut out = String::with_capacity(emit_html::emit_capacity(cells));
    emit_html_body(cells, |c| Identity.shade(c, 0), &mut out);
    out
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

    /// The frame path without the cache, as it was before the cache existed.
    fn render_frame_uncached(cfg_json: &str, frame: u32) -> Result<String, String> {
        let cfg = cfg_from_json(cfg_json)?;
        let cells = render_cells(&cfg).map_err(|e| e.message().to_string())?;
        Ok(render_frame(
            &cells,
            cfg.mode.unwrap_or(Mode::Solid),
            frame as u64,
        ))
    }

    #[test]
    fn cached_frames_match_uncached_across_config_changes() {
        let a = r#"{"text":"HI","font":"block","mode":"rainbow"}"#;
        let b = r#"{"text":"HO","font":"block","mode":"rainbow"}"#;
        let c = r#"{"text":"HO","font":"tiny","mode":"fire"}"#;
        let bad = r#"{"text":"HO","font":"nope","mode":"fire"}"#;
        for (cfg, frames) in [(a, 0..5), (b, 5..10), (c, 10..15), (a, 15..20)] {
            for f in frames {
                assert_eq!(
                    render_frame_inner(cfg, f),
                    render_frame_uncached(cfg, f),
                    "{cfg} frame {f}"
                );
            }
            assert!(render_frame_inner(bad, 0).is_err());
        }
    }

    #[test]
    fn frame_cache_is_put_back_after_a_frame() {
        let cfg = r#"{"text":"HI","font":"block","mode":"fire"}"#;
        render_frame_inner(cfg, 0).unwrap();
        render_frame_inner(cfg, 1).unwrap();
        let cached = FRAME_CACHE.with_borrow(|c| c.as_ref().map(|c| c.cfg_json.clone()));
        assert_eq!(cached.as_deref(), Some(cfg));
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
