#!/usr/bin/env bash
set -euo pipefail

WORKSPACE_PATH=${1:?"Error: missing workspace path"}
DOC_NAME=${2:?"Error: missing doc name"}
OUTPUT_FULL_PATH=${3:?"Error: missing output full path"}

source "$(git rev-parse --show-toplevel)/scripts/activate_venv.sh" venv

echo "===> Running cargo doc in '${WORKSPACE_PATH}' for ${DOC_NAME}"
mkdir -p "${OUTPUT_FULL_PATH}"
cd "${WORKSPACE_PATH}"

cargo doc --no-deps
set +e
rm target/doc/.lock || true
set -e

if [[ "${WORKSPACE_PATH}" != "./" ]]; then
  cp -R "${WORKSPACE_PATH}/target/doc" "${OUTPUT_FULL_PATH}/" || true
fi
