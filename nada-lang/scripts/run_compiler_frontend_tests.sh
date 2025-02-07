#!/usr/bin/env bash

SCRIPT_PATH="$(cd "$(dirname "${BASH_SOURCE[0]}" 2>/dev/null)" && pwd -P)"
source "$(git rev-parse --show-toplevel || echo .)/scripts/activate_venv.sh" nada-lang

# Generate operations tests
run_types_generator() {
  base_dir=$1
  target_dir=$2
  cargo run --bin operations-generator -- --mode nada-tests --base "${base_dir}" --target "${target_dir}"
}

echo "installing requirements.txt from $(pwd)"
pip3 install -r "$SCRIPT_PATH/../requirements.txt"

echo "installing nada_dsl from $(pwd)"
pip3 install "$SCRIPT_PATH/../nada_dsl"

pushd "$SCRIPT_PATH/.."
echo "running pytest in $(pwd)"
python3 -m pytest || exit 1

TMPDIR=$(mktemp -d)
run_types_generator "${SCRIPT_PATH}/../nada_dsl/nada_dsl" "${TMPDIR}"

pushd "$TMPDIR"
echo "running pytest in $(pwd)"
python3 -m pytest || exit 1
