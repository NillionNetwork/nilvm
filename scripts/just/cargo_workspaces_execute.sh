#!/usr/bin/env bash

set -euo pipefail

CARGO_WORKSPACES=${1:?"Error: Missing cargo workspaces array (e.g. \"./ ./wasm-workspace\")"}
JUST_RECIPE=${2:?"Error: missing just recipe name"}
OUTPUT_FULL_PATH=${3:-}
OPTIONS=${4:-}
INCLUDE_WORKSPACE_NAME=${5:-true}

for path in ${CARGO_WORKSPACES};
do
    workspace_name=${path/\.\//root}
    workspace_name=${workspace_name/\//\-}
    just_command="${JUST_RECIPE} ${path}"

    if [[ "${INCLUDE_WORKSPACE_NAME}" == "true" ]]; then just_command="${just_command} ${workspace_name}"; fi
    if [[ "${OUTPUT_FULL_PATH}" != "" ]]; then just_command="${just_command} ${OUTPUT_FULL_PATH}"; fi

    if [[ "${OPTIONS}" != "" ]]; then
      # shellcheck disable=SC2086
      just ${just_command} "${OPTIONS}"
    else
      # shellcheck disable=SC2086
      just ${just_command}
    fi

done