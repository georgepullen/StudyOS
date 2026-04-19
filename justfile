check:
    cargo fmt --all -- --check
    cargo clippy --workspace --all-targets -- -D warnings
    cargo test --workspace

doctor:
    cargo run -p studyos-cli -- doctor

init:
    cargo run -p studyos-cli -- init
