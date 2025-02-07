#!/bin/bash

IMAGE="$1"

SCRIPT_PATH="$(cd "$(dirname "${BASH_SOURCE[0]}" 2>/dev/null)" && pwd -P)"

# Load env vars
[[ -f "${SCRIPT_PATH}/../.env" ]] && export $(grep -E -v '^#' "${SCRIPT_PATH}/../.env" | xargs)

"$SCRIPT_PATH/docker-login.sh"

docker pull "$IMAGE"