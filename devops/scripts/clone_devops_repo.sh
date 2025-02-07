#!/usr/bin/env bash

set -o errexit

SCRIPT_PATH="$(realpath "$(dirname "$0")")"

# Constants.
REPO_ROOT="$SCRIPT_PATH/../../"
DEVOPS_REPO_DIR="$REPO_ROOT/devops/github-devops"

# Parse args.
REF="${1:-master}"

# If devops repo dir is already present, remove it. This may be the case if reusing the current
# working directory from a previous run.
if [ -d "$DEVOPS_REPO_DIR" ]; then
    rm -rf "$DEVOPS_REPO_DIR"
fi

GIT_SSH_COMMAND="ssh -o UserKnownHostsFile=/dev/null -o StrictHostKeyChecking=no" \
    git clone git@github.com:NillionNetwork/devops "$DEVOPS_REPO_DIR"

if [ "$REF" != "master" ]; then
    ( cd "$DEVOPS_REPO_DIR" && git checkout "$REF" )
fi
