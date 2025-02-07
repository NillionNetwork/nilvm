#!/bin/bash

output=$(git diff --name-only | grep Cargo.lock)
if [ $? -eq 0 ]
then
  echo "Found dirty Cargo.lock file(s):"
  echo "${output}"
  echo ""
  echo "Run 'just cargo-check-all' to ensure all Cargo.lock files are up to date"
  exit 1
fi

