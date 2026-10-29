// shout.sh — curl-friendly ANSI banner service
// Copyright (C) 2026 Ryan Lewis
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

//! Per-frame color filters for the streaming animation pipeline.

use crate::parser::Mode;
use crate::sgr::{Cell, Rgb};

pub trait Filter {
    fn shade(&self, cell: &Cell, frame: u64) -> Option<Rgb>;
}

pub struct Identity;

impl Filter for Identity {
    fn shade(&self, cell: &Cell, _frame: u64) -> Option<Rgb> {
        cell.rgb
    }
}

/// Slot sentinels used by `render_raw` to mark which cfonts paint slot a cell
/// belongs to under shader-driven modes. Shaders look up these exact RGBs in
/// `cell.rgb` to decide which per-slot variation to apply. The values are
/// arbitrary but chosen to be visually unique and extremely unlikely to
/// collide with any preset palette.
pub const SLOT_SENTINELS: [Rgb; 3] = [(254, 0, 1), (0, 254, 1), (0, 1, 254)];

/// Returns the slot index (0..N) if `cell.rgb` matches a sentinel, else None.
pub fn slot_of(cell: &Cell) -> Option<u8> {
    let rgb = cell.rgb?;
    SLOT_SENTINELS
        .iter()
        .position(|s| *s == rgb)
        .map(|i| i as u8)
}

pub struct Rainbow;

impl Rainbow {
    const HUE_PER_FRAME: u64 = 4;
    const HUE_PER_COL: u64 = 3;
    const HUE_PER_ROW: u64 = 12;
}

/// Lightness of each slot: slot 0 (front face), slot 1 (shadow), slot 2
/// (deep shadow). Saturation is always 1.0.
const RAINBOW_LIGHTNESS: [f32; 3] = [0.6, 0.35, 0.25];

/// Hue offset of each slot, in degrees. The shadow trails the front face by
/// 90° and the deep shadow by 180°.
const RAINBOW_HUE_OFFSET: [u64; 3] = [0, 90, 180];

/// `hsl_to_rgb(hue, 1.0, RAINBOW_LIGHTNESS[slot])` for every whole-degree
/// hue, indexed `[slot][hue]`. Built at compile time.
static RAINBOW_LUT: [[Rgb; 360]; 3] = {
    let mut lut = [[(0, 0, 0); 360]; 3];
    let mut slot = 0;
    while slot < 3 {
        let mut hue = 0;
        while hue < 360 {
            lut[slot][hue] = hsl_to_rgb(hue as f32, 1.0, RAINBOW_LIGHTNESS[slot]);
            hue += 1;
        }
        slot += 1;
    }
    lut
};

impl Filter for Rainbow {
    fn shade(&self, cell: &Cell, frame: u64) -> Option<Rgb> {
        if cell.ch == ' ' {
            return cell.rgb;
        }
        // Cells that carry no slot sentinel shade as the front face. Darker,
        // hue-shifted slots read as a shaded backing layer.
        let slot = slot_of(cell).map_or(0, usize::from);
        // Reduce the frame first so the sum cannot overflow.
        let hue = (frame % 360 * Self::HUE_PER_FRAME
            + cell.col as u64 * Self::HUE_PER_COL
            + cell.row as u64 * Self::HUE_PER_ROW
            + RAINBOW_HUE_OFFSET[slot])
            % 360;
        Some(RAINBOW_LUT[slot][hue as usize])
    }
}

pub struct Fire {
    /// `fire_gradient(row, rows)` for each row of the banner. The gradient
    /// depends on the banner's row count, so it is built per banner.
    gradient: Box<[Rgb]>,
}

impl Fire {
    pub fn new(rows: u16) -> Self {
        let gradient = (0..rows.max(1))
            .map(|row| fire_gradient(row, rows))
            .collect();
        Self { gradient }
    }
}

/// Bottom stop of the Fire gradient. `fire_gradient` clamps every row past
/// the banner's last row to it.
const FIRE_BOTTOM: Rgb = (255, 40, 0);

/// 3-stop vertical gradient: top=yellow, middle=orange, bottom=red.
fn fire_gradient(row: u16, rows: u16) -> Rgb {
    let rows = rows.max(1) as f32;
    // t in [0,1] bottom → top.
    let t = 1.0 - (row as f32 / (rows - 1.0).max(1.0));
    tri_gradient(t, FIRE_BOTTOM, (255, 140, 0), (255, 230, 40))
}

impl Filter for Fire {
    fn shade(&self, cell: &Cell, frame: u64) -> Option<Rgb> {
        if cell.ch == ' ' {
            return cell.rgb;
        }
        // Slot 1+ on multi-slot fonts = the shadow / behind-the-flame layer.
        // Render it as dim embers/char so the front flame reads distinctly.
        let slot = slot_of(cell);
        if matches!(slot, Some(s) if s >= 1) {
            return Some(ember(cell, frame, slot.unwrap()));
        }
        // Cells below the banner's last row are not in the table; the
        // gradient clamps them to the bottom stop.
        let (r, g, b) = self
            .gradient
            .get(usize::from(cell.row))
            .copied()
            .unwrap_or(FIRE_BOTTOM);
        let n = noise(cell.row as u32, cell.col as u32, frame as u32);
        let flick = (n as f32 / 255.0 - 0.5) * 60.0; // ±30
        let r = (r as f32 + flick).clamp(0.0, 255.0) as u8;
        let g = (g as f32 + flick * 0.4).clamp(0.0, 255.0) as u8;
        Some((r, g, b))
    }
}

/// Static dispatch over the three concrete filters so the per-cell
/// `shade` call in the hot path stays devirtualized.
pub enum Shader {
    Rainbow,
    Fire(Fire),
    Identity,
}

impl Shader {
    /// The shader a banner of `rows` rows animates with under `mode`.
    pub fn for_mode(mode: Mode, rows: u16) -> Self {
        match mode {
            Mode::Rainbow => Self::Rainbow,
            Mode::Fire => Self::Fire(Fire::new(rows)),
            Mode::Solid => Self::Identity,
        }
    }
}

impl Filter for Shader {
    fn shade(&self, cell: &Cell, frame: u64) -> Option<Rgb> {
        match self {
            Self::Rainbow => Rainbow.shade(cell, frame),
            Self::Fire(f) => f.shade(cell, frame),
            Self::Identity => Identity.shade(cell, frame),
        }
    }
}

/// Dim ember/smoke palette for background-slot cells under Fire mode. Uses
/// a slower flicker (frame/3) and a darker base so it reads as char, not
/// flame. Slot 2 is darker still for 3-slot fonts like chrome.
fn ember(cell: &Cell, frame: u64, slot: u8) -> Rgb {
    let base: Rgb = if slot >= 2 { (40, 10, 0) } else { (90, 25, 5) };
    let n = noise(cell.row as u32, cell.col as u32, (frame / 3) as u32);
    let flick = (n as f32 / 255.0 - 0.5) * 24.0; // ±12
    let r = (base.0 as f32 + flick).clamp(0.0, 255.0) as u8;
    let g = (base.1 as f32 + flick * 0.4).clamp(0.0, 255.0) as u8;
    (r, g, base.2)
}

fn tri_gradient(t: f32, a: Rgb, b: Rgb, c: Rgb) -> Rgb {
    // t=0 → a, t=0.5 → b, t=1 → c
    let t = t.clamp(0.0, 1.0);
    if t < 0.5 {
        lerp_rgb(a, b, t * 2.0)
    } else {
        lerp_rgb(b, c, (t - 0.5) * 2.0)
    }
}

fn lerp_rgb(a: Rgb, b: Rgb, t: f32) -> Rgb {
    let m = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t) as u8;
    (m(a.0, b.0), m(a.1, b.1), m(a.2, b.2))
}

fn noise(row: u32, col: u32, frame: u32) -> u8 {
    let mut x = row.wrapping_mul(0x9E3779B9)
        ^ col.wrapping_mul(0x85EBCA6B)
        ^ frame.wrapping_mul(0xC2B2AE35);
    x ^= x >> 16;
    x = x.wrapping_mul(0x7FEB352D);
    x ^= x >> 15;
    (x & 0xFF) as u8
}

pub const fn hsl_to_rgb(h: f32, s: f32, l: f32) -> Rgb {
    // HSL with h in degrees [0,360), s,l in [0,1]. Standard formula.
    let c = (1.0 - (2.0 * l - 1.0).abs()) * s;
    let hp = h / 60.0;
    // `hp.rem_euclid(2.0)`, written out because `rem_euclid` is not const.
    let r = hp % 2.0;
    let r = if r < 0.0 { r + 2.0 } else { r };
    let x = c * (1.0 - (r - 1.0).abs());
    let (r1, g1, b1) = match hp as u32 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    let m = l - c / 2.0;
    (
        ((r1 + m) * 255.0).round() as u8,
        ((g1 + m) * 255.0).round() as u8,
        ((b1 + m) * 255.0).round() as u8,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cell(ch: char, row: u16, col: u16, rgb: Option<Rgb>) -> Cell {
        Cell { ch, row, col, rgb }
    }

    #[test]
    fn identity_returns_original_rgb() {
        let c = cell('A', 0, 0, Some((10, 20, 30)));
        assert_eq!(Identity.shade(&c, 0), Some((10, 20, 30)));
        assert_eq!(Identity.shade(&c, 100), Some((10, 20, 30)));
        let bare = cell('A', 0, 0, None);
        assert_eq!(Identity.shade(&bare, 42), None);
    }

    #[test]
    fn rainbow_animates_over_frames() {
        let c = cell('A', 0, 5, None);
        let r0 = Rainbow.shade(&c, 0).unwrap();
        let r30 = Rainbow.shade(&c, 30).unwrap();
        assert_ne!(r0, r30, "rainbow must differ between frame 0 and 30");
    }

    #[test]
    fn rainbow_skips_spaces() {
        let c = cell(' ', 0, 0, None);
        assert_eq!(Rainbow.shade(&c, 0), None);
        let c2 = cell(' ', 0, 0, Some((1, 2, 3)));
        assert_eq!(Rainbow.shade(&c2, 0), Some((1, 2, 3)));
    }

    #[test]
    fn rainbow_colors_non_space_bare_chars() {
        let c = cell('╗', 0, 0, None);
        assert!(Rainbow.shade(&c, 0).is_some());
    }

    #[test]
    fn rainbow_slot_1_differs_from_slot_0() {
        // Two cells at the same position but with different slot sentinels
        // should shade differently — slot 1 is the shadow layer.
        let c0 = cell('█', 0, 0, Some(SLOT_SENTINELS[0]));
        let c1 = cell('█', 0, 0, Some(SLOT_SENTINELS[1]));
        let r0 = Rainbow.shade(&c0, 0).unwrap();
        let r1 = Rainbow.shade(&c1, 0).unwrap();
        assert_ne!(r0, r1, "slot 0 and slot 1 should shade differently");
    }

    #[test]
    fn fire_slot_1_is_dim_ember() {
        // Slot 1 cells should render as dim ember (much darker than the
        // flame at the same row).
        let flame = cell('█', 0, 0, None);
        let ember = cell('█', 0, 0, Some(SLOT_SENTINELS[1]));
        let f = Fire::new(6);
        let rf = f.shade(&flame, 0).unwrap();
        let re = f.shade(&ember, 0).unwrap();
        // flame is much brighter in red channel than ember
        assert!(rf.0 > re.0 + 100, "flame {:?} vs ember {:?}", rf, re);
    }

    #[test]
    fn slot_of_matches_sentinels() {
        assert_eq!(slot_of(&cell('x', 0, 0, Some(SLOT_SENTINELS[0]))), Some(0));
        assert_eq!(slot_of(&cell('x', 0, 0, Some(SLOT_SENTINELS[1]))), Some(1));
        assert_eq!(slot_of(&cell('x', 0, 0, Some((10, 20, 30)))), None);
        assert_eq!(slot_of(&cell('x', 0, 0, None)), None);
    }

    #[test]
    fn rainbow_is_deterministic() {
        let c = cell('A', 2, 3, None);
        assert_eq!(Rainbow.shade(&c, 7), Rainbow.shade(&c, 7));
    }

    #[test]
    fn fire_animates_and_ramps() {
        let f = Fire::new(6);
        let top = cell('█', 0, 0, None);
        let bot = cell('█', 5, 0, None);
        let t0 = f.shade(&top, 0).unwrap();
        let b0 = f.shade(&bot, 0).unwrap();
        // top is hotter (more green component for yellow-ish)
        assert!(
            t0.1 > b0.1,
            "top green {} should exceed bottom green {}",
            t0.1,
            b0.1
        );
        // animates across frames
        let t30 = f.shade(&top, 30).unwrap();
        assert_ne!(t0, t30);
    }

    #[test]
    fn fire_is_deterministic() {
        let f = Fire::new(6);
        let c = cell('█', 3, 4, None);
        assert_eq!(f.shade(&c, 9), f.shade(&c, 9));
    }

    #[test]
    fn fire_skips_spaces() {
        let f = Fire::new(6);
        assert_eq!(f.shade(&cell(' ', 0, 0, None), 0), None);
    }

    #[test]
    fn rainbow_lut_matches_hsl_to_rgb() {
        for (row, &l) in RAINBOW_LUT.iter().zip(&RAINBOW_LIGHTNESS) {
            for (hue, &rgb) in row.iter().enumerate() {
                assert_eq!(
                    rgb,
                    hsl_to_rgb(hue as f32, 1.0, l),
                    "lightness {l} hue {hue}"
                );
            }
        }
    }

    #[test]
    fn rainbow_matches_float_formula() {
        // The float maths Rainbow used before the lookup table, kept here so
        // the table path cannot drift from it.
        fn reference(cell: &Cell, frame: u64) -> Rgb {
            let base = (frame as f32 * 4.0 + cell.col as f32 * 3.0 + cell.row as f32 * 12.0)
                .rem_euclid(360.0);
            let (offset, lightness) = match slot_of(cell) {
                Some(1) => (90.0, 0.35),
                Some(2) => (180.0, 0.25),
                _ => (0.0, 0.6),
            };
            hsl_to_rgb((base + offset).rem_euclid(360.0), 1.0, lightness)
        }
        let rgbs = [
            None,
            Some((10, 20, 30)),
            Some(SLOT_SENTINELS[0]),
            Some(SLOT_SENTINELS[1]),
            Some(SLOT_SENTINELS[2]),
        ];
        for rgb in rgbs {
            for row in 0..8 {
                for col in (0..200).step_by(7) {
                    for frame in [0, 1, 29, 89, 90, 359, 360, 1_000, 123_457] {
                        let c = cell('█', row, col, rgb);
                        assert_eq!(
                            Rainbow.shade(&c, frame),
                            Some(reference(&c, frame)),
                            "rgb {rgb:?} row {row} col {col} frame {frame}"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn fire_matches_uncached_formula() {
        // The per-cell maths Fire used before the per-row table, kept here so
        // the table path cannot drift from it.
        fn reference(rows: u16, cell: &Cell, frame: u64) -> Option<Rgb> {
            if cell.ch == ' ' {
                return cell.rgb;
            }
            let slot = slot_of(cell);
            if matches!(slot, Some(s) if s >= 1) {
                return Some(ember(cell, frame, slot.unwrap()));
            }
            let rows = rows.max(1) as f32;
            let t = 1.0 - (cell.row as f32 / (rows - 1.0).max(1.0));
            let (r, g, b) = tri_gradient(t, (255, 40, 0), (255, 140, 0), (255, 230, 40));
            let n = noise(cell.row as u32, cell.col as u32, frame as u32);
            let flick = (n as f32 / 255.0 - 0.5) * 60.0;
            let r = (r as f32 + flick).clamp(0.0, 255.0) as u8;
            let g = (g as f32 + flick * 0.4).clamp(0.0, 255.0) as u8;
            Some((r, g, b))
        }
        let rgbs = [
            None,
            Some((10, 20, 30)),
            Some(SLOT_SENTINELS[0]),
            Some(SLOT_SENTINELS[1]),
            Some(SLOT_SENTINELS[2]),
        ];
        for rows in [0, 1, 2, 3, 6, 8, 13, 24, 100] {
            let fire = Fire::new(rows);
            // Two rows past the table cover the fallback.
            for row in 0..rows + 2 {
                for col in (0..200).step_by(7) {
                    for frame in [0, 1, 2, 3, 29, 360, 1_000, 123_457, u64::MAX] {
                        for rgb in rgbs {
                            for ch in [' ', '█'] {
                                let c = cell(ch, row, col, rgb);
                                assert_eq!(
                                    fire.shade(&c, frame),
                                    reference(rows, &c, frame),
                                    "rows {rows} row {row} col {col} frame {frame} rgb {rgb:?}"
                                );
                            }
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn hsl_to_rgb_primaries() {
        // 0=red, 120=green, 240=blue at s=1 l=0.5
        let red = hsl_to_rgb(0.0, 1.0, 0.5);
        let green = hsl_to_rgb(120.0, 1.0, 0.5);
        let blue = hsl_to_rgb(240.0, 1.0, 0.5);
        assert_eq!(red, (255, 0, 0));
        assert_eq!(green, (0, 255, 0));
        assert_eq!(blue, (0, 0, 255));
    }
}
