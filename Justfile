default:
    @just --list

test:
    cargo test --all

lint:
    cargo fmt --all --check
    cargo clippy --all-targets -- -D warnings
    # The Worker glue only compiles for wasm32, so lint that target too.
    RUSTFLAGS='--cfg getrandom_backend="wasm_js"' cargo clippy -p shout-worker --target wasm32-unknown-unknown -- -D warnings

fmt:
    cargo fmt --all

# Render benchmarks for shout-core (criterion). Uses the bench profile, which
# inherits the size-tuned release profile. Extra args go to criterion, for
# example `just bench -- --save-baseline main` then `-- --baseline main`.
# shout-core sets cfonts' width itself (80 columns, as in the Worker), so
# the results do not depend on the width of the pane the bench runs in.
[doc("Render benchmarks for shout-core (criterion).")]
[positional-arguments]
bench *args:
    cargo bench -p shout-core --bench render "$@"

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

# Install the pinned wrangler. No install scripts run.
worker-install:
    cd shout-worker && pnpm install --frozen-lockfile --ignore-scripts

# Build the Worker (shout-worker/build/) with worker-build.
worker-build:
    cd shout-worker && RUSTFLAGS='--cfg getrandom_backend="wasm_js"' worker-build --release

# Run the Worker locally on :8787 with esbuild in watch mode alongside.
# Ctrl-C stops both. wrangler serves web/dist/ as it changes; a Rust change
# needs a restart of `just dev`.
dev: wasm-build worker-install worker-build
    #!/usr/bin/env bash
    set -euo pipefail
    trap 'kill 0' EXIT INT TERM
    (cd web && pnpm dev) &
    # esbuild stamps index.html last, so it is the all-clear for assets.
    while [ ! -f web/dist/index.html ]; do sleep 0.1; done
    cd shout-worker && ./node_modules/.bin/wrangler dev

# Deploy the cfonts v4 preview to cfontsv4.shout.sh, from your own
# Cloudflare login. By hand only: CI deploys production and never this.
[doc("Deploy the cfonts v4 preview to cfontsv4.shout.sh (by hand, not CI).")]
deploy-cfontsv4: web-build worker-install worker-build
    cd shout-worker && ./node_modules/.bin/wrangler deploy --env cfontsv4

# Check assets, streaming and HEAD through `wrangler dev`. Needs web-build,
# worker-install and worker-build first (`just ci` does all three).
smoke:
    scripts/smoke.sh

# Measure a checkout: render benches, wasm sizes and Worker CPU per
# request, written to <out>/results.json and results.md. Run it on two
# checkouts to compare them. Paths are relative to where just runs.
# See scripts/bench-compare.sh.
[doc("Measure a checkout: render time, wasm size, Worker CPU.")]
bench-compare checkout out:
    cd "{{invocation_directory()}}" && "{{justfile_directory()}}/scripts/bench-compare.sh" "{{checkout}}" "{{out}}"

# Diff the live site against a running `just dev`, path by path.
parity:
    scripts/parity.sh

# Full CI: rebuild web assets, then run lints + tests + the Worker build.
ci: web-build lint test worker-install worker-build
