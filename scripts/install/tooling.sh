#!/usr/bin/env bash

set -ex

export PATH="$HOME/.cargo/bin:$PATH"
curl -L --proto '=https' --tlsv1.2 -sSf https://raw.githubusercontent.com/cargo-bins/cargo-binstall/main/install-from-binstall-release.sh | bash

cargo binstall "cargo-deny@~0.16" --locked
cargo binstall "just@~1.37" --locked
cargo binstall "cargo2junit@~0.1" --locked
cargo binstall "wasm-bindgen-cli@0.2.92" --locked
cargo binstall "wasm-pack@0.12.1" --locked --force
