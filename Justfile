default:
    @just --list

run:
    cargo run -p shout-server

test:
    cargo test --all

lint:
    cargo fmt --all --check
    cargo clippy --all-targets -- -D warnings

fmt:
    cargo fmt --all

# Build the browser-facing wasm bundle via wasm-pack.
wasm-build:
    RUSTFLAGS='--cfg getrandom_backend="wasm_js"' wasm-pack build shout-wasm --target web --release

# Build the TS client, embedding the freshly-built wasm.
web-build: wasm-build
    rm -rf web/src/wasm-pkg
    mkdir -p web/src/wasm-pkg
    cp shout-wasm/pkg/shout_wasm.js web/src/wasm-pkg/
    cp shout-wasm/pkg/shout_wasm.d.ts web/src/wasm-pkg/
    cp shout-wasm/pkg/shout_wasm_bg.wasm web/src/wasm-pkg/
    cp shout-wasm/pkg/shout_wasm_bg.wasm.d.ts web/src/wasm-pkg/
    cd web && pnpm install --frozen-lockfile && pnpm build

# Watch-mode esbuild for the TS client. Rewrites web/dist/ on each change;
# on its own it does NOT serve anything. Use `just dev` for the full loop.
web-dev:
    cd web && pnpm dev

# Run everything needed for local dev: esbuild in watch mode alongside the
# Rust server. Ctrl-C stops both. Server assets are `include_bytes!`'d from
# web/dist/, so to pick up a frontend change restart the server (the build
# script's rerun-if-changed on ../web/dist triggers a fast rebuild).
dev: wasm-build
    #!/usr/bin/env bash
    set -euo pipefail
    trap 'kill 0' EXIT INT TERM
    (cd web && pnpm dev) &
    # Give esbuild a moment to produce the first dist/ so the server build
    # script doesn't trip its missing-asset gate on the first `cargo run`.
    # Wait for esbuild's first build — its manifest is the last file written.
    while [ ! -f web/dist/index.html ] || [ ! -f web/dist/og.png ] || [ ! -f web/dist/_app/manifest.txt ]; do sleep 0.1; done
    cargo run -p shout-server

# Full CI: rebuild web assets, then run lints + tests + release build.
ci: web-build lint test
    cargo build --release -p shout-server
