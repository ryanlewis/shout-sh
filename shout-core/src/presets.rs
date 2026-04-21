// shout.sh — curl-friendly ANSI banner service
// Copyright (C) 2026 Ryan Lewis
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

//! Named color palettes. Each preset is an ordered list of 1–3 hex stops
//! consumed by cfonts' gradient path. At render time the renderer takes
//! `stops[..font.color_count]` — extra stops are silently dropped so every
//! preset reads as "close enough" on every font.

pub struct Preset {
    pub name: &'static str,
    pub stops: &'static [&'static str],
}

/// Curated palettes. Order is the display order in `/presets`.
/// Keep in sync with `web/src/ui/PresetPicker.ts::PRESETS` — the TS list is
/// hand-maintained, not generated, and a mismatch only surfaces at runtime.
pub const PRESETS: &[Preset] = &[
    Preset {
        name: "sunset",
        stops: &["#ffb347", "#ff3366"],
    },
    Preset {
        name: "ocean",
        stops: &["#00d4ff", "#0047b3"],
    },
    Preset {
        name: "mint",
        stops: &["#a8f5c8", "#00a080"],
    },
    Preset {
        name: "candy",
        stops: &["#ff6fd8", "#7b2cff"],
    },
    Preset {
        name: "matrix",
        stops: &["#6dff8f", "#0a4020"],
    },
    Preset {
        name: "mono",
        stops: &["#ffffff", "#606060"],
    },
    Preset {
        name: "neon",
        stops: &["#ff00ea", "#00eaff"],
    },
    Preset {
        name: "ember",
        stops: &["#ffd866", "#ff3b1a"],
    },
];

pub fn is_preset(name: &str) -> bool {
    resolve(name).is_some()
}

pub fn resolve(name: &str) -> Option<&'static Preset> {
    PRESETS.iter().find(|p| p.name == name)
}

pub fn list_newline() -> String {
    PRESETS
        .iter()
        .map(|p| p.name)
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_presets_have_hex_stops() {
        for p in PRESETS {
            assert!(!p.stops.is_empty(), "{} has no stops", p.name);
            for s in p.stops {
                assert!(s.starts_with('#') && s.len() == 7, "{s} is not #rrggbb");
                assert!(
                    s[1..].chars().all(|c| c.is_ascii_hexdigit()),
                    "{s} has non-hex digits"
                );
            }
        }
    }

    #[test]
    fn resolve_known_preset() {
        assert!(resolve("sunset").is_some());
        assert!(resolve("unknown").is_none());
    }

    #[test]
    fn is_preset_matches() {
        assert!(is_preset("neon"));
        assert!(!is_preset("red"));
    }

    #[test]
    fn preset_names_do_not_collide_with_colors_or_modes() {
        for p in PRESETS {
            assert!(
                !crate::render::is_color(p.name),
                "{} collides with color",
                p.name
            );
            assert!(
                crate::parser::Mode::from_token(p.name).is_none(),
                "{} collides with mode",
                p.name
            );
            assert!(
                !crate::fonts::is_font(p.name),
                "{} collides with font",
                p.name
            );
        }
    }
}
