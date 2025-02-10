#!/bin/bash

SCRIPT_PATH="$(cd "$(dirname "${BASH_SOURCE[0]}" 2>/dev/null)" && pwd -P)"
source "$(git rev-parse --show-toplevel || echo .)/scripts/activate_venv.sh" venv

pip3 install -r "$SCRIPT_PATH/../requirements.txt"

echo "installing nada_dsl from $(pwd)"
pip3 install "$SCRIPT_PATH/../nada_dsl"

cd "$SCRIPT_PATH/.." || exit 1

mkdir -p ../target/junit/pynadac
cargo test -p pynadac  -- -Z unstable-options --format json --report-time | cargo2junit > ../target/junit/pynadac/pynadac.xml
