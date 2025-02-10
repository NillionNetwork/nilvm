#!/usr/bin/env bash

set -e
SCRIPT_PATH="$(cd "$(dirname "${BASH_SOURCE[0]}" 2>/dev/null)" && pwd -P)"

export WASM_BINDGEN_TEST_ONLY_WEB="1"

# Enable this to debug the tests using the browser by hand, run a single test and open http://127.0.0.1:8000/
# export NO_HEADLESS="1"
export WASM_BINDGEN_KEEP_DEBUG="1"
#export WASM_BINDGEN_SPLIT_LINKED_MODULES="1"
$HOME/.cargo/bin/wasm-bindgen-test-runner $@
