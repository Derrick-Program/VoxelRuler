set shell := ["bash", "-eu", "-o", "pipefail", "-c"]
default: help

@help:
  just --list --unsorted

@fmt:
  cargo fmt

@lint:
  cargo clippy --all-targets --all-features -- -D warnings

@test:
  cargo test --all-features

@run:
  RUST_BACKTRACE=full RUSTFLAGS="--cfg tokio_unstable" cargo run

@pack:
  cargo packager --release