#!/usr/bin/env bash

set -ex

SCRIPT_PATH="$(cd "$(dirname "${BASH_SOURCE[0]}" 2>/dev/null)" && pwd -P)"
TOOLCHAIN=$(cat "$SCRIPT_PATH/../../rust-toolchain.toml" | grep "channel" | sed "s|channel *= *\"\(.*\)\"|\1|g")

command -v rustup || curl https://sh.rustup.rs -sSf | sh -s -- -y --profile minimal --default-toolchain $TOOLCHAIN
export PATH="$HOME/.cargo/bin:$PATH"

echo -e "\nTo make persistent changes, add \"\$HOME/.cargo/bin\" to your PATH\n"
rustup component add cargo
rustup component add clippy
rustup component add rustfmt
rustup component add rust-src

rustup target add wasm32-unknown-unknown
