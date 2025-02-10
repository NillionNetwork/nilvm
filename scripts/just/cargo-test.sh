#!/usr/bin/env bash

TEST_PATH=$1
REPORT_NAME=$2
OUTPUT_FULL_PATH=$3
TEST_OPTIONS="${4:-}"
TEST_BINARY_OPTIONS="${5:-}"

echo "===> Running cargo test in '${TEST_PATH}'"
source scripts/activate_venv.sh venv
set -e -o pipefail
export CARGO_TARGET_DIR="$(realpath "${CARGO_TARGET_DIR}")"

mkdir -p "$OUTPUT_FULL_PATH" &&
  cd "${TEST_PATH}" &&
  cargo test $TEST_OPTIONS -- -Z unstable-options --format json --report-time $TEST_BINARY_OPTIONS | cargo2junit >$OUTPUT_FULL_PATH/$REPORT_NAME.xml
