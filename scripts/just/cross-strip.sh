#!/bin/bash
set -e
set -x

PACKAGE="${1:-missing package positional parameter}"
TARGET="${2:-missing target positional parameter}"

# on my linux machine, it adds 'unknown' to the binary name
TARGET_PREFIX=$(echo "$TARGET" | sed 's/-unknown//')

COMMAND="${TARGET_PREFIX}-strip target/cross-target/${TARGET}/${TARGET}/release/${PACKAGE}"

cross-util run --target "$TARGET" -- "sh -c \"$COMMAND\""
