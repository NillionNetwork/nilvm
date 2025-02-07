#!/usr/bin/env bash

set -o errexit

SCRIPT_PATH="$(realpath "$(dirname "$0")")"

# Constants
REPO_ROOT="$SCRIPT_PATH"/../../..

# Parse args.
SDK_PATH="${1:?SDK_PATH is required}"

while read -r PROGRAM; do
    echo "Running build-program.sh script with SDK path '$SDK_PATH' and program '$PROGRAM'" >&2

    if "$REPO_ROOT"/devops/scripts/build-program.sh "$SDK_PATH" "$PROGRAM"; then
        echo "Successfully ran build-program.sh." >&2
    else
        echo "An error occurred running build-program.sh"
        exit 1
    fi
done < <(find "$REPO_ROOT"/devops/load-tests/programs -maxdepth 1 -type f -name "*.py")
