#!/usr/bin/env bash

set -euo pipefail -o errexit

SCRIPT_PATH="$(realpath "$(dirname "$0")")"

# Parse args.
SDK_PATH="${1:?SDK_PATH is required}"
PROGRAM_PATH="${2:?PROGRAM_PATH is required}"

if [ ! -d "$SDK_PATH" ]; then
  echo "SDK path '$SDK_PATH' does not exist" >&2
  exit 1
fi

if [ ! -f "$PROGRAM_PATH" ]; then
  echo "Program path '$PROGRAM_PATH' does not exist" >&2
  exit 1
fi

# Capture the local nada-dsl version and install it. We can't know for sure what version the SDK we're using needs
# but usually we will be testing against close to HEAD to odds are these are the same.
git submodule update --init --recursive
NADA_DSL_VERSION=$(cat "${SCRIPT_PATH}/../../nada-lang/nada_dsl/pyproject.toml" | grep version | sed -n 's/version = "\(.*\)"/\1/p')
echo "Installing nada-dsl version ${NADA_DSL_VERSION}"

# Install nada-dsl package.
# shellcheck disable=SC1091
source "$(git rev-parse --show-toplevel || echo .)/scripts/activate_venv.sh" venv
uv pip install "nada-dsl==${NADA_DSL_VERSION}"

# Compile nada program.
PROGRAM_DIR="$(dirname "$PROGRAM_PATH")"
"$SDK_PATH"/pynadac -t "$PROGRAM_DIR" "$PROGRAM_PATH"
