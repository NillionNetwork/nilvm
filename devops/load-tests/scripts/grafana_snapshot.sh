#!/usr/bin/env bash

SCRIPT_PATH="$(realpath "$(dirname "$0")")"

START_TIME="${1:?START_TIME is required}"
END_TIME="${2:?END_TIME is required}"
ENV_NAME="${3:?ENV_NAME is required}"

# shellcheck disable=SC1091
source "$(git rev-parse --show-toplevel || echo .)/scripts/activate_venv.sh" venv &>/dev/null
uv pip install -r "$SCRIPT_PATH"/load-test-manager/requirements.txt &>/dev/null

: "${GRAFANA_NILLION_NETWORK_UUID:?is not set}"
: "${GRAFANA_TOKEN:?is not set}"
: "${GRAFANA_URL:?is not set}"

"$SCRIPT_PATH"/load-test-manager/load-test-manager grafana-snapshot \
  "$GRAFANA_NILLION_NETWORK_UUID" \
  "$START_TIME" \
  "$END_TIME" \
  "$ENV_NAME"
