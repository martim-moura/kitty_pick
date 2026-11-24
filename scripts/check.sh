#!/bin/sh
set -eu
cd "$(dirname "$0")/.."
cargo fmt --all -- --check
cargo clippy --all-targets --all-features --no-deps -- -D warnings
cargo test --all-targets --locked
cargo build --locked
python3 scripts/pty_smoke.py target/debug/kitty-pick
if command -v fish >/dev/null 2>&1; then
    fish --no-config --no-execute integration/kitty-pick-cd.fish
fi
git diff --check
