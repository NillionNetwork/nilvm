#!/usr/bin/env bash

set -euo pipefail

WORKSPACE_PATH=${1:?"Error: Missing workspace path"}
shift

echo "===> Running cargo checks in '${WORKSPACE_PATH}"

cd "${WORKSPACE_PATH}"

echo "=> cargo check"
cargo check --bins --tests --examples "$@"
