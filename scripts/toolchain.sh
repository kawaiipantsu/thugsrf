#!/bin/sh
set -eu
if command -v rustup >/dev/null 2>&1; then
    rustup toolchain install 1.98.1 --profile minimal --component clippy --component rustfmt
elif [ -x "$HOME/.cargo/bin/rustup" ]; then
    "$HOME/.cargo/bin/rustup" toolchain install 1.98.1 --profile minimal --component clippy --component rustfmt
else
    thugsrf_installer=$(mktemp)
    trap 'rm -f "$thugsrf_installer"' EXIT HUP INT TERM
    curl --proto '=https' --tlsv1.2 -fsSL https://sh.rustup.rs -o "$thugsrf_installer"
    sh "$thugsrf_installer" -y --profile minimal --default-toolchain 1.98.1 --component clippy --component rustfmt
fi
