set shell := ["bash", "-eu", "-o", "pipefail", "-c"]
default: help
export TAG := "v" + `grep -m 1 '^version' Cargo.toml | cut -d '"' -f 2`

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

@show-latest-tag:
    git tag --sort=-creatordate | head -n 1

@show-all-tags:
    git tag --sort=-creatordate

@add-tag:
    git tag {{TAG}} && git push origin {{TAG}}

@check-tags:
    @echo "Git Tag: {{TAG}}"

@remove-tag tag:
    @echo "正在刪除本地 Tag: {{tag}}..."
    git tag -d {{tag}}
    @echo "正在刪除遠端 Tag: {{tag}}..."
    git push origin --delete {{tag}}
    @echo "✅ Tag {{tag}} 已完全移除。"