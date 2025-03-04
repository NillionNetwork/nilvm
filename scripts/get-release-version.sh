#!/usr/bin/env bash

SCRIPT_PATH="$(cd "$(dirname "${BASH_SOURCE[0]}" 2>/dev/null)" && pwd -P)"

# shellcheck disable=SC1091
source "$SCRIPT_PATH"/include/release_management.sh

get_release_version
