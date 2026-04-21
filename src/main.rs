// Phase-0 spike: prove out cfonts's Rust API and inspect its output shape so we
// can judge whether phase-2 animation (post-filtering pre-colored ANSI) is
// tractable. Not shipping code — will be replaced by the real service.

use cfonts::{render, Align, BgColors, Colors, Env, Fonts, Options, Rgb};

fn banner(label: &str) {
    println!("\n===== {} =====", label);
}

fn dump(label: &str, opts: Options) {
    banner(label);
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| render(opts)));
    match result {
        Ok(out) => {
            println!("lines: {}", out.lines);
            println!("vec entries: {}", out.vec.len());
            println!("--- text ---");
            println!("{}", out.text);
            println!("--- first line (debug, shows escapes) ---");
            if let Some(first) = out.vec.first() {
                println!("{:?}", first);
            }
        }
        Err(_) => {
            println!("PANICKED");
        }
    }
}

fn main() {
    dump(
        "block / single color (red)",
        Options {
            text: "HI".into(),
            font: Fonts::FontBlock,
            colors: vec![Colors::Red],
            ..Options::default()
        },
    );

    dump(
        "block / two-color gradient NAMED (red -> blue)",
        Options {
            text: "HI".into(),
            font: Fonts::FontBlock,
            gradient: vec!["red".into(), "blue".into()],
            ..Options::default()
        },
    );

    dump(
        "block / two-color gradient HEX (#ff0000 -> #0000ff)",
        Options {
            text: "HI".into(),
            font: Fonts::FontBlock,
            gradient: vec!["#ff0000".into(), "#0000ff".into()],
            ..Options::default()
        },
    );

    dump(
        "block / transition gradient HEX",
        Options {
            text: "HI".into(),
            font: Fonts::FontBlock,
            gradient: vec!["#ff0000".into(), "#00ff00".into(), "#0000ff".into()],
            transition_gradient: true,
            ..Options::default()
        },
    );

    dump(
        "tiny / candy",
        Options {
            text: "Hello World".into(),
            font: Fonts::FontTiny,
            colors: vec![Colors::Candy],
            ..Options::default()
        },
    );

    dump(
        "3d / rgb custom",
        Options {
            text: "3D".into(),
            font: Fonts::Font3d,
            colors: vec![Colors::Rgb(Rgb::Val(255, 100, 50))],
            background: BgColors::Transparent,
            align: Align::Left,
            ..Options::default()
        },
    );

    // Key phase-2 question: does Env::Browser give uncolored output we can
    // re-color each frame?
    dump(
        "block / Env::Browser (inspect for uncolored output)",
        Options {
            text: "HI".into(),
            font: Fonts::FontBlock,
            colors: vec![Colors::Red],
            env: Env::Browser,
            ..Options::default()
        },
    );
}
