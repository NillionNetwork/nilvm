#!/bin/bash
CONTAINER_NAME="${1:?}"
CLEAN_BRANCH="$(scripts/util_clean_branch_name.sh)"
echo "${CONTAINER_NAME}-${CLEAN_BRANCH}-${BUILD_NUMBER:-local}"
