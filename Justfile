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

# Watch-mode dev server for the TS client. The Rust server in debug mode
# proxies /_app/* and / to this port — see shout-server/src/server.rs.
web-dev:
    cd web && pnpm dev

# Full CI: rebuild web assets, then run lints + tests + release build.
ci: web-build lint test
    cargo build --release -p shout-server
