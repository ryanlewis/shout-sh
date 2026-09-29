// shout.sh — curl-friendly ANSI banner service
// Copyright (C) 2026 Ryan Lewis
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

//! Render benchmarks. Run with `just bench`.
//!
//! `render_cells` is the one-off cost of a request: cfonts plus
//! `sgr::parse`. `emit_shaded` is the bulk of the per-frame cost of an
//! animated stream. `emit_html` is the same for a frame of the browser
//! playground, so its config has `browser` on, as shout-wasm's does. The
//! other configs match what the Worker builds for a curl request (`browser`
//! off). cfonts then wraps at the terminal width, so run through `just
//! bench`, which detaches the terminal to get the Worker's 80 columns.

use std::hint::black_box;

use criterion::{Criterion, Throughput, criterion_group, criterion_main};
use shout_core::emit_html;
use shout_core::parser::{Mode, RenderConfig};
use shout_core::render::{emit_shaded, render_cells};
use shout_core::sgr::{self, Cell};
use shout_core::shader::{Filter, Shader};

const SHORT: &str = "shout";
const LONG: &str = "the quick brown fox jumps over the lazy dog";

// `simple` has one colour slot, `block` (the default font) has two.
const FONTS: [&str; 2] = ["simple", "block"];

fn cfg(text: &str, font: &str, mode: Mode) -> RenderConfig {
    RenderConfig {
        text: text.into(),
        font: font.into(),
        mode: Some(mode),
        ..Default::default()
    }
}

fn bench_render_cells(c: &mut Criterion) {
    let mut g = c.benchmark_group("render_cells");
    for font in FONTS {
        for (len, text) in [("short", SHORT), ("long", LONG)] {
            // Rainbow is the mode a stream renders with most. It also takes
            // the multi-slot sentinel path on `block`.
            let config = cfg(text, font, Mode::Rainbow);
            g.bench_function(format!("{font}/{len}"), |b| {
                b.iter(|| render_cells(black_box(&config)).unwrap())
            });
        }
    }
    // Fire on a one-slot font takes cfonts' transition_gradient path.
    let config = cfg(LONG, "simple", Mode::Fire);
    g.bench_function("fire/simple/long", |b| {
        b.iter(|| render_cells(black_box(&config)).unwrap())
    });
    g.finish();
}

/// Rows and columns the grid covers, for the benchmark name.
fn size(cells: &[Cell]) -> (u16, u16) {
    let cols = cells.iter().map(|c| c.col + 1).max().unwrap_or(0);
    (cols, sgr::row_count(cells))
}

fn bench_frame(c: &mut Criterion, name: &str, mode: Mode) {
    // "shout.sh" in `block` is 70 columns by 6 glyph rows (420 cells), below
    // 2 rows of top padding: a banner close to a full terminal width.
    let cells = render_cells(&cfg("shout.sh", "block", mode)).unwrap();
    let (cols, rows) = size(&cells);
    // The shader the Worker builds, so the per-cell enum match is timed too.
    let filter = Shader::for_mode(mode, rows);
    let mut g = c.benchmark_group("emit_shaded");
    g.throughput(Throughput::Elements(cells.len() as u64));
    g.bench_function(format!("{name}/{cols}x{rows}"), |b| {
        // Step the frame so Rainbow and Fire do not repeat one frame.
        let mut frame = 0u64;
        b.iter(|| {
            frame = frame.wrapping_add(1);
            emit_shaded(black_box(&cells), &filter, black_box(frame))
        })
    });
    g.finish();
}

fn bench_emit_shaded(c: &mut Criterion) {
    bench_frame(c, "identity", Mode::Solid);
    bench_frame(c, "rainbow", Mode::Rainbow);
    bench_frame(c, "fire", Mode::Fire);
}

fn bench_emit_html(c: &mut Criterion) {
    // The bench_frame grid, built and emitted as shout-wasm's render_frame does.
    let config = RenderConfig {
        browser: true,
        ..cfg("shout.sh", "block", Mode::Rainbow)
    };
    let cells = render_cells(&config).unwrap();
    let (cols, rows) = size(&cells);
    let filter = Shader::for_mode(Mode::Rainbow, rows);
    let mut g = c.benchmark_group("emit_html");
    g.throughput(Throughput::Elements(cells.len() as u64));
    g.bench_function(format!("rainbow/{cols}x{rows}"), |b| {
        let mut frame = 0u64;
        b.iter(|| {
            frame = frame.wrapping_add(1);
            let frame = black_box(frame);
            emit_html::emit_body(black_box(&cells), |c| filter.shade(c, frame))
        })
    });
    g.finish();
}

criterion_group!(
    benches,
    bench_render_cells,
    bench_emit_shaded,
    bench_emit_html
);
criterion_main!(benches);
