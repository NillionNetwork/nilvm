#!/usr/bin/env bash

set -euo pipefail

ROOT_DIR="$(git rev-parse --show-toplevel)"
CARGO_CHECK="${ROOT_DIR}/scripts/cargo-check.sh"

source "${ROOT_DIR}/scripts/activate_venv.sh" venv

"$CARGO_CHECK" "${ROOT_DIR}" --benches --features=bench
