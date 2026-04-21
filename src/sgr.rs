// shout.sh — curl-friendly ANSI banner service
// Copyright (C) 2026 Ryan Lewis
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

//! SGR-aware scanner over cfonts output.
//!
//! cfonts emits three cell shapes (see spike-findings.md):
//!   - `\x1b[31m...\x1b[39m`              16-color runs (solid mode)
//!   - `\x1b[38;2;R;G;Bm.\x1b[39m`        truecolor per-char (gradient mode)
//!   - bare glyphs (box-drawing chars in solid mode, spaces everywhere)
//!
//! Rows are delimited by `\n`. Columns advance one per char (cfonts is
//! monospace); we do not special-case width — wide glyphs aren't in use.

use std::fmt::Write as _;

pub type Rgb = (u8, u8, u8);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cell {
    pub ch: char,
    pub rgb: Option<Rgb>,
    pub row: u16,
    pub col: u16,
}

/// Parse cfonts output into a flat vec of cells (one per visible char).
/// Newlines are dropped from the output but advance `row`.
pub fn parse(input: &str) -> Vec<Cell> {
    let mut cells = Vec::with_capacity(input.len());
    let mut cur: Option<Rgb> = None;
    let mut row: u16 = 0;
    let mut col: u16 = 0;
    let chars: Vec<char> = input.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c == '\x1b' && chars.get(i + 1) == Some(&'[') {
            // scan params until a letter terminator
            let mut j = i + 2;
            while j < chars.len() && !chars[j].is_ascii_alphabetic() {
                j += 1;
            }
            if j < chars.len() {
                let term = chars[j];
                let params: String = chars[i + 2..j].iter().collect();
                if term == 'm' {
                    apply_sgr(&params, &mut cur);
                }
                i = j + 1;
                continue;
            } else {
                // unterminated; treat rest as literal
                i += 1;
                continue;
            }
        }
        if c == '\n' {
            row += 1;
            col = 0;
            i += 1;
            continue;
        }
        cells.push(Cell {
            ch: c,
            rgb: cur,
            row,
            col,
        });
        col += 1;
        i += 1;
    }
    cells
}

fn apply_sgr(params: &str, cur: &mut Option<Rgb>) {
    // Accept empty "" as reset (per ECMA-48: CSI m == CSI 0 m).
    if params.is_empty() {
        *cur = None;
        return;
    }
    let parts: Vec<&str> = params.split(';').collect();
    let mut k = 0;
    while k < parts.len() {
        let n: u16 = parts[k].parse().unwrap_or(0);
        match n {
            0 | 39 => {
                *cur = None;
                k += 1;
            }
            38 => {
                // 38;2;R;G;B (truecolor) or 38;5;N (256-color)
                if let Some(submode) = parts.get(k + 1).and_then(|s| s.parse::<u16>().ok()) {
                    if submode == 2 && parts.len() >= k + 5 {
                        let r = parts[k + 2].parse().unwrap_or(0);
                        let g = parts[k + 3].parse().unwrap_or(0);
                        let b = parts[k + 4].parse().unwrap_or(0);
                        *cur = Some((r, g, b));
                        k += 5;
                        continue;
                    }
                    if submode == 5 && parts.len() >= k + 3 {
                        k += 3;
                        continue;
                    }
                }
                k += 1;
            }
            30..=37 => {
                *cur = Some(ansi16_rgb(n - 30, false));
                k += 1;
            }
            90..=97 => {
                *cur = Some(ansi16_rgb(n - 90, true));
                k += 1;
            }
            _ => k += 1,
        }
    }
}

fn ansi16_rgb(idx: u16, bright: bool) -> Rgb {
    // xterm-like palette.
    const NORMAL: [Rgb; 8] = [
        (0, 0, 0),
        (170, 0, 0),
        (0, 170, 0),
        (170, 85, 0),
        (0, 0, 170),
        (170, 0, 170),
        (0, 170, 170),
        (170, 170, 170),
    ];
    const BRIGHT: [Rgb; 8] = [
        (85, 85, 85),
        (255, 85, 85),
        (85, 255, 85),
        (255, 255, 85),
        (85, 85, 255),
        (255, 85, 255),
        (85, 255, 255),
        (255, 255, 255),
    ];
    if bright {
        BRIGHT[idx as usize]
    } else {
        NORMAL[idx as usize]
    }
}

/// Count rows spanned by the cell grid. 0 if empty.
pub fn row_count(cells: &[Cell]) -> u16 {
    cells.last().map(|c| c.row + 1).unwrap_or(0)
}

/// Emit a frame as bytes: `\n`-separated rows, coalesced SGR runs, bare
/// chars passed through verbatim. Does not emit any cursor-control codes
/// (the caller owns that).
pub fn emit(cells: &[Cell]) -> String {
    if cells.is_empty() {
        return String::new();
    }
    let rows = row_count(cells);
    let mut out = String::with_capacity(cells.len() * 4);
    let mut idx = 0;
    for row in 0..rows {
        let mut open: Option<Rgb> = None;
        // Find cells for this row. Cells are already row-sorted.
        while idx < cells.len() && cells[idx].row == row {
            let cell = &cells[idx];
            match (open, cell.rgb) {
                (Some(o), Some(c)) if o == c => {
                    out.push(cell.ch);
                }
                (_, Some(c)) => {
                    if open.is_some() {
                        out.push_str("\x1b[39m");
                    }
                    let _ = write!(out, "\x1b[38;2;{};{};{}m", c.0, c.1, c.2);
                    out.push(cell.ch);
                    open = Some(c);
                }
                (Some(_), None) => {
                    out.push_str("\x1b[39m");
                    open = None;
                    out.push(cell.ch);
                }
                (None, None) => {
                    out.push(cell.ch);
                }
            }
            idx += 1;
        }
        if open.is_some() {
            out.push_str("\x1b[39m");
        }
        if row + 1 < rows {
            out.push('\n');
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_bare_chars() {
        let cells = parse("ab\nc");
        assert_eq!(cells.len(), 3);
        assert_eq!(
            cells[0],
            Cell {
                ch: 'a',
                rgb: None,
                row: 0,
                col: 0
            }
        );
        assert_eq!(
            cells[1],
            Cell {
                ch: 'b',
                rgb: None,
                row: 0,
                col: 1
            }
        );
        assert_eq!(
            cells[2],
            Cell {
                ch: 'c',
                rgb: None,
                row: 1,
                col: 0
            }
        );
    }

    #[test]
    fn parse_16color_run() {
        let cells = parse("\x1b[31mAB\x1b[39mC");
        assert_eq!(cells.len(), 3);
        assert_eq!(cells[0].rgb, Some((170, 0, 0)));
        assert_eq!(cells[1].rgb, Some((170, 0, 0)));
        assert_eq!(cells[2].rgb, None);
    }

    #[test]
    fn parse_truecolor_per_char() {
        let cells = parse("\x1b[38;2;1;2;3m█\x1b[39m\x1b[38;2;4;5;6m╗\x1b[39m");
        assert_eq!(cells.len(), 2);
        assert_eq!(cells[0].rgb, Some((1, 2, 3)));
        assert_eq!(cells[0].ch, '█');
        assert_eq!(cells[1].rgb, Some((4, 5, 6)));
        assert_eq!(cells[1].ch, '╗');
    }

    #[test]
    fn parse_mixed_colored_and_bare() {
        let cells = parse("\x1b[31m██\x1b[39m╗");
        assert_eq!(cells[0].rgb, Some((170, 0, 0)));
        assert_eq!(cells[1].rgb, Some((170, 0, 0)));
        assert_eq!(cells[2].rgb, None);
        assert_eq!(cells[2].ch, '╗');
    }

    #[test]
    fn rows_tracked() {
        let cells = parse("ab\ncd");
        assert_eq!(cells[0].row, 0);
        assert_eq!(cells[2].row, 1);
        assert_eq!(cells[2].col, 0);
        assert_eq!(cells[3].col, 1);
    }

    #[test]
    fn emit_roundtrips_truecolor() {
        let input = "\x1b[38;2;10;20;30m█\x1b[39m\x1b[38;2;40;50;60m█\x1b[39m";
        let cells = parse(input);
        let out = emit(&cells);
        // same semantic content (may coalesce vs not), reparse must match
        let back = parse(&out);
        assert_eq!(back, cells);
    }

    #[test]
    fn emit_coalesces_runs() {
        let input = "\x1b[38;2;1;2;3m█\x1b[39m\x1b[38;2;1;2;3m█\x1b[39m";
        let cells = parse(input);
        let out = emit(&cells);
        // should open once and close once
        assert_eq!(out.matches("\x1b[38;2;1;2;3m").count(), 1);
        assert_eq!(out.matches("\x1b[39m").count(), 1);
    }

    #[test]
    fn emit_handles_bare_and_colored_mixed() {
        let input = "\x1b[31m█\x1b[39m╗\x1b[31m█\x1b[39m";
        let cells = parse(input);
        let out = emit(&cells);
        let back = parse(&out);
        assert_eq!(back, cells);
    }

    #[test]
    fn emit_preserves_row_count() {
        let input = "a\nb\nc";
        let cells = parse(input);
        let out = emit(&cells);
        assert_eq!(out.lines().count(), 3);
        assert_eq!(row_count(&cells), 3);
    }

    #[test]
    fn reset_0_closes_color() {
        let cells = parse("\x1b[31mA\x1b[0mB");
        assert_eq!(cells[0].rgb, Some((170, 0, 0)));
        assert_eq!(cells[1].rgb, None);
    }

    #[test]
    fn unknown_sgr_ignored() {
        let cells = parse("\x1b[1mA\x1b[22mB");
        assert_eq!(cells.len(), 2);
        assert_eq!(cells[0].rgb, None);
    }

    #[test]
    fn empty_input() {
        assert_eq!(parse("").len(), 0);
        assert_eq!(emit(&[]), "");
    }
}
