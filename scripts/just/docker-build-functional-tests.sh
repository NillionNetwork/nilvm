#!/usr/bin/env bash

set -euo pipefail

ROOT_DIR="$(git rev-parse --show-toplevel)"
cd "$ROOT_DIR"
target_dir="target/debug/docker/"

source scripts/activate_venv.sh venv

# Build once so we see error messages
cargo test -p functional --no-run

# Now re-run again, which should be very fast, and take out the executable path
mkdir -p "$target_dir"
binary_path=$(cargo test -p functional --no-run 2>&1 | sed -n 's/.*Executable.*(\(.*\)).*/\1/p')
cp "$binary_path" ${target_dir}/functional-tests
strip "${target_dir}/functional-tests"
cp "$(which cargo2junit)" "${target_dir}/cargo2junit"

./scripts/docker-build.sh nillion-functional-tests "$target_dir" --with-cache
