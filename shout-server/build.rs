// The shout-server binary embeds the TS client via `include_bytes!`. If the
// web bundle is missing, the macro errors are cryptic — fail loudly with a
// clear pointer instead.

use std::path::PathBuf;

fn main() {
    let dist = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("web")
        .join("dist");

    println!("cargo:rerun-if-changed=../web/dist");

    let required = [
        "index.html",
        "about.html",
        "privacy.html",
        "main.js",
        "main.css",
        "shout_wasm.js",
        "shout_wasm_bg.wasm",
        "favicon.svg",
        "og.png",
    ];
    let missing: Vec<&str> = required
        .iter()
        .copied()
        .filter(|f| !dist.join(f).exists())
        .collect();

    if !missing.is_empty() {
        eprintln!(
            "\nshout-server build gate: web/dist/ missing {missing:?}\n\
             run `just web-build` from the repo root before building the server.\n"
        );
        std::process::exit(1);
    }
}
