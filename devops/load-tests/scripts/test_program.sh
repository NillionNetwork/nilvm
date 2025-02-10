#!/usr/bin/env bash

PY_CLIENT_WHEEL="py_nillion_client-0.1.1-cp37-abi3-manylinux_2_17_x86_64.manylinux2014_x86_64.whl"

PROGRAM_ID="${1:?PROGRAM_ID is required}"
SDK_PATH="${2:?SDK_PATH is required}"
TEST_PATH="${3:?TEST_PATH is required}"

if [ ! -f "$TEST_PATH" ]; then
  echo "Test path '$TEST_PATH' does not exist" >&2
  exit 1
fi

# shellcheck disable=SC1091
source "$(git rev-parse --show-toplevel || echo .)/scripts/activate_venv.sh" venv

TEST_DIR="$(dirname "$TEST_PATH")"

uv pip install -r "$TEST_DIR"/requirements.txt
uv pip install "$SDK_PATH"/"$PY_CLIENT_WHEEL"

# Run multi-party program.
PROGRAM_ID="$PROGRAM_ID" python "$TEST_PATH"
