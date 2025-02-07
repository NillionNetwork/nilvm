#!/bin/bash

valid_lockfiles=(Cargo.lock wasm-workspace/Cargo.lock)
lockfiles=$(git ls-files | grep Cargo.lock)
if [ $(echo "$lockfiles" | wc -l) -ne ${#valid_lockfiles[@]} ]; then
  echo "Found the following Cargo.lock files:"
  echo "${lockfiles}"
  echo ""
  echo "But valid lock files are only: ${valid_lockfiles[@]}"
  exit 1
fi
