check:
    cargo fmt --all -- --check
    cargo clippy --workspace -- -D warnings
