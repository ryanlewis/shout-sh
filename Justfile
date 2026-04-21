default:
    @just --list

run:
    cargo run

test:
    cargo test

lint:
    cargo fmt --check
    cargo clippy --all-targets -- -D warnings

fmt:
    cargo fmt

ci: lint test
    cargo build --release
