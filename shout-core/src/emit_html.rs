// shout.sh — curl-friendly ANSI banner service
// Copyright (C) 2026 Ryan Lewis
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

//! HTML emitter. Mirrors `sgr::emit_with` but produces
//! `<pre class="shout-frame">...<span style="color:#rrggbb">run</span>...</pre>`.
//!
//! This is the ONLY place cell text meets user-visible HTML. Every cell char
//! is escaped here — callers MUST NOT concatenate user text into the output
//! buffer and MUST NOT skip the emitter for "plain" runs.

use crate::sgr::{Cell, Rgb, row_count};

const SPAN_OPEN_PREFIX: &str = "<span style=\"color:#";
const SPAN_OPEN_SUFFIX: &str = "\">";
const SPAN_CLOSE: &str = "</span>";

/// Most bytes `emit_html_body` writes for a grid. Under `Rainbow` and `Fire`
/// almost every cell opens a new span, so the worst cell is a close, an open,
/// and the longest escape (`&quot;`, longer than any 4-byte char). Each row
/// adds a close and a `\n`. Mirrors `sgr::emit_capacity`.
pub fn emit_capacity(cells: &[Cell]) -> usize {
    const OPEN: usize = SPAN_OPEN_PREFIX.len() + "rrggbb".len() + SPAN_OPEN_SUFFIX.len();
    const CLOSE: usize = SPAN_CLOSE.len();
    const CHAR: usize = "&quot;".len();
    let rows = usize::from(row_count(cells));
    cells.len() * (CLOSE + OPEN + CHAR) + rows * (CLOSE + 1)
}

/// Emit the full `<pre>...</pre>` wrapper around a frame.
pub fn emit_html_with<F>(cells: &[Cell], color_of: F, out: &mut String)
where
    F: Fn(&Cell) -> Option<Rgb>,
{
    out.push_str("<pre class=\"shout-frame\">");
    emit_html_body(cells, color_of, out);
    out.push_str("</pre>");
}

/// Emit only the inner content (no `<pre>` wrapper). Useful when the wrapper
/// is already in the DOM and only the frame body is swapped each tick.
pub fn emit_html_body<F>(cells: &[Cell], color_of: F, out: &mut String)
where
    F: Fn(&Cell) -> Option<Rgb>,
{
    if cells.is_empty() {
        return;
    }
    let rows = row_count(cells);
    let mut idx = 0;
    for row in 0..rows {
        let mut open: Option<Rgb> = None;
        while idx < cells.len() && cells[idx].row == row {
            let cell = &cells[idx];
            let rgb = color_of(cell);
            match (open, rgb) {
                (Some(o), Some(c)) if o == c => {
                    push_escaped(out, cell.ch);
                }
                (_, Some(c)) => {
                    if open.is_some() {
                        out.push_str(SPAN_CLOSE);
                    }
                    push_span_open(out, c);
                    push_escaped(out, cell.ch);
                    open = Some(c);
                }
                (Some(_), None) => {
                    out.push_str(SPAN_CLOSE);
                    open = None;
                    push_escaped(out, cell.ch);
                }
                (None, None) => {
                    push_escaped(out, cell.ch);
                }
            }
            idx += 1;
        }
        if open.is_some() {
            out.push_str(SPAN_CLOSE);
        }
        if row + 1 < rows {
            out.push('\n');
        }
    }
}

fn push_span_open(out: &mut String, (r, g, b): Rgb) {
    out.push_str(SPAN_OPEN_PREFIX);
    push_hex(out, r);
    push_hex(out, g);
    push_hex(out, b);
    out.push_str(SPAN_OPEN_SUFFIX);
}

/// Push `n` as two lowercase hex digits.
fn push_hex(out: &mut String, n: u8) {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    out.push(char::from(DIGITS[usize::from(n >> 4)]));
    out.push(char::from(DIGITS[usize::from(n & 0xf)]));
}

fn push_escaped(out: &mut String, ch: char) {
    match ch {
        '&' => out.push_str("&amp;"),
        '<' => out.push_str("&lt;"),
        '>' => out.push_str("&gt;"),
        '"' => out.push_str("&quot;"),
        c => out.push(c),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sgr::parse;

    fn body(cells: &[Cell]) -> String {
        let mut s = String::new();
        emit_html_body(cells, |c| c.rgb, &mut s);
        s
    }

    #[test]
    fn wrapper_around_frame() {
        let cells = parse("ab");
        let mut s = String::new();
        emit_html_with(&cells, |c| c.rgb, &mut s);
        assert!(s.starts_with("<pre class=\"shout-frame\">"));
        assert!(s.ends_with("</pre>"));
    }

    #[test]
    fn escapes_lt_gt_amp_quot() {
        // Simulate user text flowing through cfonts → cells unescaped.
        let cells = vec![
            Cell {
                ch: '<',
                rgb: None,
                row: 0,
                col: 0,
            },
            Cell {
                ch: '>',
                rgb: None,
                row: 0,
                col: 1,
            },
            Cell {
                ch: '&',
                rgb: None,
                row: 0,
                col: 2,
            },
            Cell {
                ch: '"',
                rgb: None,
                row: 0,
                col: 3,
            },
        ];
        let b = body(&cells);
        assert_eq!(b, "&lt;&gt;&amp;&quot;");
        assert!(!b.contains('<'));
        assert!(!b.contains('>'));
    }

    #[test]
    fn script_tag_cannot_escape() {
        let txt = "<script>alert(1)</script>";
        let cells: Vec<Cell> = txt
            .chars()
            .enumerate()
            .map(|(i, ch)| Cell {
                ch,
                rgb: None,
                row: 0,
                col: i as u16,
            })
            .collect();
        let b = body(&cells);
        assert!(
            !b.contains("<script"),
            "raw <script must never reach output: {b}"
        );
        assert!(b.contains("&lt;script&gt;"));
    }

    #[test]
    fn bare_cells_emit_plain_text_no_span() {
        let cells = parse("ab\nc");
        let b = body(&cells);
        assert_eq!(b, "ab\nc");
        assert!(!b.contains("<span"));
    }

    #[test]
    fn coalesces_same_color_run() {
        // Two truecolor chars same RGB → one span.
        let input = "\x1b[38;2;1;2;3m█\x1b[39m\x1b[38;2;1;2;3m█\x1b[39m";
        let cells = parse(input);
        let b = body(&cells);
        assert_eq!(b.matches("<span").count(), 1);
        assert_eq!(b.matches("</span>").count(), 1);
        assert!(b.contains("color:#010203"));
    }

    #[test]
    fn switches_color_closes_span() {
        let input = "\x1b[38;2;1;2;3m█\x1b[39m\x1b[38;2;4;5;6m█\x1b[39m";
        let cells = parse(input);
        let b = body(&cells);
        assert_eq!(b.matches("<span").count(), 2);
        assert!(b.contains("color:#010203"));
        assert!(b.contains("color:#040506"));
    }

    #[test]
    fn colored_to_bare_closes_span() {
        let input = "\x1b[38;2;1;2;3m█\x1b[39mX";
        let cells = parse(input);
        let b = body(&cells);
        assert_eq!(b.matches("<span").count(), 1);
        assert_eq!(b.matches("</span>").count(), 1);
        assert!(b.ends_with('X'));
    }

    #[test]
    fn row_boundaries_close_open_span() {
        // Colored char on row 0, bare char on row 1 — must close span before newline.
        let cells = vec![
            Cell {
                ch: '█',
                rgb: Some((1, 2, 3)),
                row: 0,
                col: 0,
            },
            Cell {
                ch: 'x',
                rgb: None,
                row: 1,
                col: 0,
            },
        ];
        let b = body(&cells);
        assert!(b.contains("</span>\nx"), "got: {b:?}");
    }

    #[test]
    fn push_span_open_matches_format() {
        for n in 0..=255u8 {
            let rgb = (n, 255 - n, n / 2);
            let mut out = String::new();
            push_span_open(&mut out, rgb);
            assert_eq!(
                out,
                format!(
                    "<span style=\"color:#{:02x}{:02x}{:02x}\">",
                    rgb.0, rgb.1, rgb.2
                )
            );
        }
    }

    #[test]
    fn emit_capacity_covers_worst_case() {
        // Every cell a new color and the longest escape, over two rows.
        let cells: Vec<Cell> = (0..20u16)
            .map(|i| Cell {
                ch: '"',
                rgb: Some((255, 255, i as u8)),
                row: i / 10,
                col: i % 10,
            })
            .collect();
        let mut s = String::new();
        emit_html_body(&cells, |c| c.rgb, &mut s);
        assert!(
            s.len() <= emit_capacity(&cells),
            "{} > {}",
            s.len(),
            emit_capacity(&cells)
        );
    }

    #[test]
    fn empty_cells_empty_body() {
        assert_eq!(body(&[]), "");
    }

    #[test]
    fn stable_snapshot() {
        let cells = vec![
            Cell {
                ch: 'A',
                rgb: Some((255, 0, 0)),
                row: 0,
                col: 0,
            },
            Cell {
                ch: 'B',
                rgb: Some((255, 0, 0)),
                row: 0,
                col: 1,
            },
            Cell {
                ch: 'C',
                rgb: None,
                row: 0,
                col: 2,
            },
        ];
        let b = body(&cells);
        assert_eq!(b, "<span style=\"color:#ff0000\">AB</span>C");
    }
}
