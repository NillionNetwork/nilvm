#!/bin/bash

SCRIPT_PATH="$(cd "$(dirname "${BASH_SOURCE[0]}" 2>/dev/null)" && pwd -P)"

run_types_generator() {
  target_dir=$1
  cargo run --bin operations-generator -- --mode nada-types --base "" --target "${target_dir}"
}

run_types_generator "${SCRIPT_PATH}/../nada_dsl/nada_dsl/nada_types"
