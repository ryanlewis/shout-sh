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
use shout_core::shader::Shader;

/// Most bytes per second one stream may send. The largest banner a normal
/// phrase makes (three words in `3d`, about 58 KB a frame) needs 0.6 MB/s
/// at the default 10 fps.
pub const MAX_BYTES_PER_SEC: u64 = 1_000_000;
/// Most bytes one stream may send in total. The same banner at the default
/// 10 fps for 60 s is about 35 MB.
pub const MAX_STREAM_BYTES: u64 = 64_000_000;

/// Lower `fps`, then `timeout`, so that a stream whose redraws are
/// `frame_bytes` long stays under `MAX_BYTES_PER_SEC` and
/// `MAX_STREAM_BYTES`. Never raises either value (an `fps` of 0 is read
/// as 1), and never caps below 1. A frame bigger than
/// `MAX_BYTES_PER_SEC` still gets 1 fps.
///
/// `frame_bytes` comes from frame 0. Later frames vary with the shader:
/// fire frames measured up to about 4% bigger, so the ceilings can be
/// passed by that much.
pub fn cap(frame_bytes: usize, fps: u32, timeout: u32) -> (u32, u32) {
    let frame = (frame_bytes as u64).max(1);
    let fps_cap = (MAX_BYTES_PER_SEC / frame).max(1);
    let fps = u64::from(fps.max(1)).min(fps_cap);
    // The stream ticks every `1000 / fps` ms, rounded down, and sends a
    // frame on every tick before the deadline. Size the timeout from that
    // tick so the rounding does not add frames past the budget.
    let tick_ms = 1000 / fps;
    let frames = MAX_STREAM_BYTES / frame;
    let timeout_cap = (frames * tick_ms / 1000).max(1);
    let timeout = u64::from(timeout).min(timeout_cap);
    // Both are at most their u32 inputs, so the casts cannot truncate.
    (fps as u32, timeout as u32)
}

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
    /// Frame 0, rendered up front to size the stream. Taken on first step.
    first: String,
    shader: Shader,
    up: String,
    tick_ms: u64,
    timeout_ms: u64,
    /// `(fps, timeout)` after `cap`, when it lowered either one.
    capped: Option<(u32, u32)>,
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
            .field("capped", &self.capped)
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
        let first = emit_shaded(&cells, &shader, 0);
        // Every later frame is the cursor-up plus a frame about this size.
        let asked = (cfg.fps.max(1), cfg.timeout);
        let (fps, timeout) = cap(up.len() + first.len(), asked.0, asked.1);
        Ok(Self {
            cells,
            first,
            shader,
            up,
            tick_ms: u64::from(1000 / fps),
            timeout_ms: u64::from(timeout) * 1000,
            capped: ((fps, timeout) != asked).then_some((fps, timeout)),
            frame: 0,
            start_ms: 0,
            next_due_ms: 0,
            finished: false,
        })
    }

    /// How long the stream runs after its first frame, after `cap`.
    pub fn timeout_ms(&self) -> u64 {
        self.timeout_ms
    }

    /// `(fps, timeout)` the stream runs at, if `cap` lowered what was asked.
    pub fn capped(&self) -> Option<(u32, u32)> {
        self.capped
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
                std::mem::take(&mut self.first),
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
