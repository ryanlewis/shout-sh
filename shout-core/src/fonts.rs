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

use cfonts::Fonts;

/// Canonical lowercase font names in display order. `simpleblock` /
/// `simple3d` are the canonical spellings; hyphenated aliases are accepted
/// by `resolve` for user ergonomics.
pub const FONTS: &[&str] = &[
    "block",
    "slick",
    "tiny",
    "grid",
    "pallet",
    "shade",
    "chrome",
    "simple",
    "simpleblock",
    "3d",
    "simple3d",
    "huge",
    "console",
];

pub fn is_font(name: &str) -> bool {
    resolve(name).is_some()
}

pub fn resolve(name: &str) -> Option<Fonts> {
    match name {
        "block" => Some(Fonts::FontBlock),
        "slick" => Some(Fonts::FontSlick),
        "tiny" => Some(Fonts::FontTiny),
        "grid" => Some(Fonts::FontGrid),
        "pallet" => Some(Fonts::FontPallet),
        "shade" => Some(Fonts::FontShade),
        "chrome" => Some(Fonts::FontChrome),
        "simple" => Some(Fonts::FontSimple),
        "simpleblock" | "simple-block" => Some(Fonts::FontSimpleBlock),
        "3d" => Some(Fonts::Font3d),
        "simple3d" | "simple-3d" => Some(Fonts::FontSimple3d),
        "huge" => Some(Fonts::FontHuge),
        "console" => Some(Fonts::FontConsole),
        _ => None,
    }
}

pub fn list_newline() -> String {
    FONTS.join("\n")
}

/// How many colors the font's JSON consumes. Drives preset-gradient
/// truncation so a two-stop palette doesn't over-color a single-color font
/// (and a three-stop preset doesn't under-color chrome).
pub fn color_count(name: &str) -> usize {
    match name {
        "chrome" => 3,
        "block" | "slick" | "grid" | "pallet" | "shade" | "huge" | "3d" => 2,
        // simple, simpleblock, simple3d, tiny, console
        _ => 1,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_names_resolve() {
        for n in FONTS {
            assert!(resolve(n).is_some(), "failed to resolve {n}");
        }
    }

    #[test]
    fn hyphenated_aliases() {
        assert!(resolve("simple-block").is_some());
        assert!(resolve("simple-3d").is_some());
    }

    #[test]
    fn unknown_is_none() {
        assert!(resolve("standard").is_none());
        assert!(resolve("").is_none());
    }

    #[test]
    fn color_count_matches_cfonts_json() {
        // Values sourced from cfonts' bundled font JSONs at pin time.
        assert_eq!(color_count("chrome"), 3);
        assert_eq!(color_count("block"), 2);
        assert_eq!(color_count("3d"), 2);
        assert_eq!(color_count("tiny"), 1);
        assert_eq!(color_count("simple"), 1);
        assert_eq!(color_count("simple3d"), 1);
        assert_eq!(color_count("console"), 1);
    }
}
