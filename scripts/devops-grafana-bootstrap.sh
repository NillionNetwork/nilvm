#!/bin/bash -e

echo "this script will bootstrap grafana with mattias' python helper."

SCRIPT_DIR="${1:?You must specify the path to your devops repository, it should be relative to this file or absolute path}"

SCRIPT_PATH="${SCRIPT_DIR}/observability/grafana/create-dashboards.py"
PYREQS_PATH="${SCRIPT_DIR}/observability/grafana/requirements.txt"

if [ ! -f "$SCRIPT_PATH"]; then
  echo "missing devops repo file: create-dashboards.py at expected path"
  exit 1
fi

if [ ! -f "$PYREQS_PATH"]; then
  echo "missing devops repo file: requirements.txt at expected path"
  exit 1
fi

ENV_DIR=$(mktemp -d)
KEY_NAME="bootstrap-$(date +%s)"

GRAFANA_SERVER="http://localhost:3000"
export GRAFANA_SERVER

RESPONSE=$(curl -X POST -H "Content-Type: application/json" -d "{
  \"name\": \"$KEY_NAME\",
  \"role\": \"Admin\"
}" "$GRAFANA_SERVER/api/auth/keys")

GRAFANA_TOKEN="$(jq -r '.key' <<<"$RESPONSE")"
export GRAFANA_TOKEN

if [ -z "${GRAFANA_TOKEN}" ]; then
  echo "Failed to create API key"
  exit 1
fi

echo "Using ${ENV_DIR} to install python environment"
python3 -m venv $ENV_DIR
source $ENV_DIR/bin/activate

echo "Installing python dependencies"
uv pip install -r "$PYREQS_PATH"

echo "Running create-dashboards script"
$SCRIPT_PATH
rm -rf $ENV_DIR
