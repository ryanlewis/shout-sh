// shout.sh — curl-friendly ANSI banner service
// Copyright (C) 2026 Ryan Lewis
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

//! Frame generation and pacing for animated banners. No clock or timer
//! lives here: the caller passes the current time in milliseconds and
//! sleeps for whatever `Step::Wait` asks. That keeps the framing testable
//! on the host with a fake clock.

use shout_core::parser::{Mode, RenderConfig};
use shout_core::render::{RenderError, emit_shaded, render_cells};
use shout_core::sgr::{self, Cell, ansi};
use shout_core::shader::{Filter, Fire, Identity, Rainbow};

/// What the stream should do next.
#[derive(Debug, PartialEq, Eq)]
pub enum Step {
    /// Send this frame now.
    Frame(String),
    /// Nothing is due. Sleep this many milliseconds and ask again.
    Wait(u64),
    /// The timeout passed. Send this reset sequence and stop.
    End(String),
    /// The reset sequence was already sent.
    Done,
}

pub struct Animation {
    cells: Vec<Cell>,
    shader: Shader,
    up: String,
    tick_ms: u64,
    timeout_ms: u64,
    frame: u64,
    start_ms: u64,
    next_due_ms: u64,
    finished: bool,
}

impl std::fmt::Debug for Animation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Animation")
            .field("tick_ms", &self.tick_ms)
            .field("timeout_ms", &self.timeout_ms)
            .field("frame", &self.frame)
            .finish_non_exhaustive()
    }
}

impl Animation {
    pub fn new(cfg: &RenderConfig) -> Result<Self, RenderError> {
        let cells = render_cells(cfg)?;
        let rows = sgr::row_count(&cells);
        let shader = Shader::for_mode(cfg.mode.unwrap_or(Mode::Solid), rows);
        // \x1b[0A is invalid; skip cursor-up if the banner had no rows.
        let up = if rows > 0 {
            format!("\x1b[{rows}A\r")
        } else {
            String::from("\r")
        };
        Ok(Self {
            cells,
            shader,
            up,
            tick_ms: u64::from(1000 / cfg.fps.max(1)),
            timeout_ms: u64::from(cfg.timeout) * 1000,
            frame: 0,
            start_ms: 0,
            next_due_ms: 0,
            finished: false,
        })
    }

    /// Advance the stream. The first call sends frame 0 straight away and
    /// starts the clock; later frames fall on `start + n * tick` until the
    /// timeout. A late caller skips the missed ticks rather than bursting.
    pub fn step(&mut self, now_ms: u64) -> Step {
        if self.finished {
            return Step::Done;
        }
        if self.frame == 0 {
            self.start_ms = now_ms;
            self.next_due_ms = now_ms + self.tick_ms;
            self.frame = 1;
            return Step::Frame(format!(
                "{}{}{}{}",
                ansi::HIDE_CURSOR,
                ansi::CLEAR_SCREEN,
                ansi::CURSOR_HOME,
                emit_shaded(&self.cells, &self.shader, 0),
            ));
        }
        let deadline = self.start_ms + self.timeout_ms;
        if now_ms >= deadline {
            self.finished = true;
            return Step::End(format!("{}{}\n", ansi::SGR_RESET, ansi::SHOW_CURSOR));
        }
        if now_ms < self.next_due_ms {
            return Step::Wait(self.next_due_ms.min(deadline) - now_ms);
        }
        let out = format!(
            "{}{}",
            self.up,
            emit_shaded(&self.cells, &self.shader, self.frame)
        );
        self.frame += 1;
        self.next_due_ms += self.tick_ms;
        if self.next_due_ms <= now_ms {
            let missed = (now_ms - self.start_ms) / self.tick_ms + 1;
            self.next_due_ms = self.start_ms + missed * self.tick_ms;
        }
        Step::Frame(out)
    }
}

/// Static dispatch over the three concrete filters so the per-cell
/// `shade` call in the hot path stays devirtualized.
enum Shader {
    Rainbow,
    Fire(Fire),
    Identity,
}

impl Shader {
    fn for_mode(mode: Mode, rows: u16) -> Self {
        match mode {
            Mode::Rainbow => Self::Rainbow,
            Mode::Fire => Self::Fire(Fire { rows }),
            Mode::Solid => Self::Identity,
        }
    }
}

impl Filter for Shader {
    fn shade(&self, cell: &Cell, frame: u64) -> Option<sgr::Rgb> {
        match self {
            Self::Rainbow => Rainbow.shade(cell, frame),
            Self::Fire(f) => f.shade(cell, frame),
            Self::Identity => Identity.shade(cell, frame),
        }
    }
}
