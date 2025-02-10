#!/usr/bin/env bash

SCRIPT_PATH="$(cd "$(dirname "${BASH_SOURCE[0]}" 2>/dev/null)" && pwd -P)"

RELEASE_VERSION="${1:?RELEASE_VERSION is not set}"
FORCE="${2:-false}"

# shellcheck disable=SC1091
source "$(git rev-parse --show-toplevel || echo .)/scripts/activate_venv.sh" venv &>/dev/null
uv pip install -r "$SCRIPT_PATH"/../tools/release-manager/requirements.txt &>/dev/null

if [[ "$FORCE" == "true" ]]; then
  OPTS="--force"
else
  OPTS=
fi

"$SCRIPT_PATH"/../tools/release-manager/release-manager delete-release $OPTS "$RELEASE_VERSION"
