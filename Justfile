check:
    cargo fmt --all -- --check
    cargo clippy --workspace -- -D warnings

purity:
    bash ci/check_model_purity.sh
