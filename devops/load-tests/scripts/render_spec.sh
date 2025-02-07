#!/usr/bin/env bash

SCRIPT_PATH="$(realpath "$(dirname "$0")")"

SPEC_PATH="${1:?SPEC_PATH is required}"
MAX_FLOW_DURATION="${2:-}"
MAX_TEST_DURATION="${3:-}"
OPERATION_INPUT_SIZE="${4:-}"
WORKERS="${5:-}"

if [[ -n "$MAX_FLOW_DURATION" ]]; then
  OPTS="--max-flow-duration $MAX_FLOW_DURATION"
fi

if [[ -n "$MAX_TEST_DURATION" ]]; then
  OPTS="$OPTS --max-test-duration $MAX_TEST_DURATION"
fi

if [[ -n "$OPERATION_INPUT_SIZE" ]]; then
  OPTS="$OPTS --operation-input-size $OPERATION_INPUT_SIZE"
fi

if [[ -n "$WORKERS" ]]; then
  OPTS="$OPTS --workers $WORKERS"
fi

if [[ -n "$REQUIRED_STARTING_BALANCE" ]]; then
  OPTS="$OPTS --required-starting-balance $REQUIRED_STARTING_BALANCE"
fi

# shellcheck disable=SC1091
source "$(git rev-parse --show-toplevel || echo .)/scripts/activate_venv.sh" venv
uv pip install -r "$SCRIPT_PATH"/load-test-manager/requirements.txt

# shellcheck disable=SC2086
"$SCRIPT_PATH"/load-test-manager/load-test-manager render-spec $OPTS "$SPEC_PATH"
echo "Rendered spec '$SPEC_PATH' with options '$OPTS'."

# Validate output of render-spec command.
RENDERED_SPEC="$SPEC_PATH.rendered"

if [ ! -e "$RENDERED_SPEC" ]; then
  echo "Rendered spec '$RENDERED_SPEC' does not exist. Generation must have failed." >&2
  exit 1
fi

echo "Showing spec '$RENDERED_SPEC':" >&2
cat "$RENDERED_SPEC"
