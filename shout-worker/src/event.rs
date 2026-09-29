// shout.sh — curl-friendly ANSI banner service
// Copyright (C) 2026 Ryan Lewis
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

//! One Analytics Engine data point per request.
//!
//! Every field is a `&'static str` from a fixed list, so nothing the
//! caller typed (banner text, raw URL, query values) can reach the
//! dataset. Unknown fonts and presets are recorded as an empty string.
//!
//! Layout (the order is the query schema, do not reorder):
//!
//! | column  | value                                                |
//! | ------- | ---------------------------------------------------- |
//! | index1  | route                                                |
//! | blob1   | route: matched pattern, e.g. `/fonts/{name}`         |
//! | blob2   | render kind: `static`, `json`, `animated` or empty   |
//! | blob3   | font name from the built-in list, or empty           |
//! | blob4   | mode: `default`, `solid`, `rainbow`, `fire` or empty |
//! | blob5   | preset name from the built-in list, or empty         |
//! | blob6   | render error kind (see `error_label`), or empty      |
//! | double1 | HTTP status                                          |
//! | double2 | duration in ms (see note below)                      |
//! | double3 | frames sent (streams only, else 0)                   |
//!
//! Duration: the Workers clock only moves on I/O, so for a static render
//! it is usually 0. For a stream it is the wall time until the stream
//! ended or the client went away.

use shout_core::fonts;
use shout_core::parser::{Mode, RenderConfig};
use shout_core::presets;
use shout_core::render::RenderError;

/// Route label for the catch-all banner renderer.
pub const ROUTE_RENDER: &str = "/render";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RenderKind {
    Static,
    Json,
    Animated,
}

impl RenderKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Static => "static",
            Self::Json => "json",
            Self::Animated => "animated",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Event {
    pub route: &'static str,
    pub status: u16,
    pub kind: Option<RenderKind>,
    pub font: &'static str,
    pub mode: &'static str,
    pub preset: &'static str,
    pub error: &'static str,
}

impl Event {
    pub fn route(route: &'static str, status: u16) -> Self {
        Self {
            route,
            status,
            kind: None,
            font: "",
            mode: "",
            preset: "",
            error: "",
        }
    }

    pub fn render(route: &'static str, status: u16, kind: RenderKind, cfg: &RenderConfig) -> Self {
        Self {
            route,
            status,
            kind: Some(kind),
            font: font_label(&cfg.font),
            mode: mode_label(cfg.mode),
            preset: preset_label(&cfg.preset),
            error: "",
        }
    }

    pub fn with_error(mut self, err: &RenderError) -> Self {
        self.error = error_label(err);
        self
    }

    pub fn blobs(&self) -> [&'static str; 6] {
        [
            self.route,
            self.kind.map(RenderKind::as_str).unwrap_or(""),
            self.font,
            self.mode,
            self.preset,
            self.error,
        ]
    }

    pub fn doubles(&self, duration_ms: u64, frames: u64) -> [f64; 3] {
        [f64::from(self.status), duration_ms as f64, frames as f64]
    }
}

/// `cfg.font` can come from `?font=`, which the parser does not validate,
/// so map it onto the built-in list instead of copying it.
fn font_label(font: &str) -> &'static str {
    fonts::FONTS
        .iter()
        .copied()
        .find(|f| *f == font)
        .unwrap_or("")
}

fn mode_label(mode: Option<Mode>) -> &'static str {
    match mode {
        None => "default",
        Some(Mode::Solid) => "solid",
        Some(Mode::Rainbow) => "rainbow",
        Some(Mode::Fire) => "fire",
    }
}

/// Same reason as `font_label`: `?preset=` is caller text.
fn preset_label(preset: &str) -> &'static str {
    presets::PRESETS
        .iter()
        .map(|p| p.name)
        .find(|n| *n == preset)
        .unwrap_or("")
}

/// Chosen from the variant, never from the message text.
fn error_label(err: &RenderError) -> &'static str {
    match err {
        RenderError::UnknownFont => "unknown_font",
        RenderError::UnknownColor => "unknown_color",
        RenderError::UnknownPreset => "unknown_preset",
        RenderError::EmptyText => "empty_text",
    }
}
