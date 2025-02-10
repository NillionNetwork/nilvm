#!/bin/bash

SCRIPT_PATH="$(realpath "$(dirname "$0")")"
cd "$SCRIPT_PATH/.." || exit 1

source "$(git rev-parse --show-toplevel)/scripts/activate_venv.sh" venv

if [ "$1" == "" ]; then
  RUST_LOG=warn cargo run -p auto-tests
else
  RUST_LOG=warn cargo run -p auto-tests -- "$1"
fi
