#!/usr/bin/env bash
set -euo pipefail

WORKSPACE_PATH=${1:?"Error: missing workspace path"}

source "$(git rev-parse --show-toplevel)/scripts/activate_venv.sh" venv

echo "===> Running cargo clippy in '${WORKSPACE_PATH}'"

cd "${WORKSPACE_PATH}"

cargo clippy -Zunstable-options -- -Dwarnings
