#!/usr/bin/env bash

SCRIPT_PATH="$(realpath "$(dirname "$0")")"

REPORT_PATH="${1:?REPORT_PATH is required}"

LOAD_TOOL_PATH="$SCRIPT_PATH"/../../../tools/load-tool

# Install requirements for report generation.
# shellcheck disable=SC1091
source "$(git rev-parse --show-toplevel || echo .)/scripts/activate_venv.sh" venv
uv pip install -r "$LOAD_TOOL_PATH"/reports/scripts/requirements.txt
uv pip install ipykernel==6.29.0
python3 -m ipykernel install --user --name=venv

while read -r REPORT_JSON; do
  REPORT_NOTEBOOK=${REPORT_JSON//.json/.ipynb}
  papermill "$LOAD_TOOL_PATH"/reports/load.ipynb "$REPORT_NOTEBOOK" -p results_file_path "$REPORT_JSON"
  jupyter nbconvert --to html "$REPORT_NOTEBOOK"
done < <(find "$REPORT_PATH" -maxdepth 1 -type f -name "report*.json")
