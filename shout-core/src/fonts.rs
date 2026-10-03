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

use cfonts::Font;

/// Canonical lowercase font names in display order. `simpleblock` is the
/// canonical spelling; the hyphenated alias is accepted by `resolve` for
/// user ergonomics.
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
    "huge",
    "console",
];

pub fn is_font(name: &str) -> bool {
    resolve(name).is_some()
}

pub fn resolve(name: &str) -> Option<Font> {
    match name {
        "block" => Some(Font::Block),
        "slick" => Some(Font::Slick),
        "tiny" => Some(Font::Tiny),
        "grid" => Some(Font::Grid),
        "pallet" => Some(Font::Pallet),
        "shade" => Some(Font::Shade),
        "chrome" => Some(Font::Chrome),
        "simple" => Some(Font::Simple),
        "simpleblock" | "simple-block" => Some(Font::SimpleBlock),
        "3d" => Some(Font::Font3D),
        // cfonts v4 dropped simple3d. Old links render in `simple`, the
        // nearest font, rather than as a banner reading "SIMPLE3D/...".
        "simple3d" | "simple-3d" => Some(Font::Simple),
        "huge" => Some(Font::Huge),
        "console" => Some(Font::Console),
        _ => None,
    }
}

pub fn list_newline() -> String {
    FONTS.join("\n")
}

/// How many color slots the font has. Drives preset-gradient truncation so
/// a two-stop palette doesn't over-color a single-color font (and a
/// three-stop preset doesn't under-color chrome).
pub fn color_count(name: &str) -> usize {
    resolve(name).map_or(1, |font| font.get_font().colors())
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
    }

    #[test]
    fn simple3d_falls_back_to_simple() {
        assert!(!FONTS.contains(&"simple3d"));
        assert_eq!(resolve("simple3d"), Some(Font::Simple));
        assert_eq!(resolve("simple-3d"), Some(Font::Simple));
    }

    #[test]
    fn unknown_is_none() {
        assert!(resolve("standard").is_none());
        assert!(resolve("").is_none());
    }

    #[test]
    fn color_count_matches_cfonts() {
        assert_eq!(color_count("chrome"), 3);
        assert_eq!(color_count("block"), 2);
        assert_eq!(color_count("3d"), 2);
        assert_eq!(color_count("tiny"), 1);
        assert_eq!(color_count("simple"), 1);
        assert_eq!(color_count("simpleblock"), 1);
        assert_eq!(color_count("huge"), 2);
        assert_eq!(color_count("console"), 1);
    }
}
