#!/bin/bash

TARGET_DIR="$1"
SCRIPT_PATH="$(cd "$(dirname "${BASH_SOURCE[0]}" 2>/dev/null)" && pwd -P)"

run_types_generator() {
  cargo run --bin operations-generator -- --mode markdown-table --base "" --target "${TARGET_DIR}/operations.md"
  cp ${SCRIPT_PATH}/../docs/styles/operations.css ${TARGET_DIR}/
  pandoc -s ${TARGET_DIR}/operations.md --css=operations.css --metadata title=Operations -o ${TARGET_DIR}/operations.html
}

run_types_generator
