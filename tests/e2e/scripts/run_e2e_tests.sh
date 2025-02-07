#!/bin/bash

TEST_OPTIONS=${1:-""}

set -e

SCRIPT_PATH="$(cd "$(dirname "${BASH_SOURCE[0]}" 2>/dev/null)" && pwd -P)"
source "$(git rev-parse --show-toplevel || echo .)/scripts/activate_venv.sh" venv

echo "===> Running end-to-end tests in "
cd $SCRIPT_PATH/../../.. || exit 1
just cargo-test tests/e2e e2e "$(pwd)/target/junit/e2e"  "${TEST_OPTIONS}"
