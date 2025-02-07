#!/bin/bash
BRANCH_NAME="${BRANCH_NAME:-"$(git rev-parse --abbrev-ref HEAD)"}"
CLEAN_BRANCH=${BRANCH_NAME//\//-}
CLEAN_BRANCH=${CLEAN_BRANCH//\~/-}
echo "$CLEAN_BRANCH"
