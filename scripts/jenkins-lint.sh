#!/bin/bash
set -e
set -o pipefail

SCRIPT_PATH="$(cd "$(dirname "${BASH_SOURCE[0]}" 2>/dev/null)" && pwd -P)"

file=${1:?missing pipeline file}

# Load env vars
# shellcheck disable=SC2046
[[ -f "${SCRIPT_PATH}/../.env" ]] && export $(grep -E -v '^#' "${SCRIPT_PATH}/../.env" | xargs)

JENKINS_HOME="https://jenkins-internal.nilogy.xyz"

TMPFILE=$(mktemp)
curl -s --user "${JENKINS_USER}:${JENKINS_TOKEN}" \
    -X POST -F "jenkinsfile=<${file}" "${JENKINS_HOME}/pipeline-model-converter/validate" >>"$TMPFILE"

if grep --quiet "Errors encountered" "$TMPFILE"; then
    echo "$file has errors"
    cat "$TMPFILE"
    exit 1
fi

# crumb generator
# CRUMB=$(curl -s "${JENKINS_HOME}/crumbIssuer/api/xml?xpath=concat(//crumbRequestField,\":\",//crumb)" -u "${JENKINS_USER}:${JENKINS_TOKEN}")
