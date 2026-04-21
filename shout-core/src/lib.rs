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

//! Pure rendering pipeline shared by the server and the wasm playground.
//! Zero async / HTTP / tokio deps — depends only on `cfonts`.

pub mod emit_html;
pub mod fonts;
pub mod parser;
pub mod presets;
pub mod render;
pub mod sanitize;
pub mod sgr;
pub mod shader;

pub use parser::{Mode, RenderConfig, parse};
pub use render::{RenderError, banner, emit_shaded, render_cells, render_config};
