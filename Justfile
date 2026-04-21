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

ci: lint test
    cargo build --release -p shout-server
