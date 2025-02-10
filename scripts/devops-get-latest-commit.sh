#!/usr/bin/env bash

set -o errexit

get_devops_master_sha() {
    local response_file
    response_file=$(mktemp)

    local http_code
    if ! http_code=$(curl \
        -H "Accept: application/vnd.github+json" \
        -H "Authorization: Bearer $GITHUB_TOKEN" \
        -H "X-GitHub-Api-Version: 2022-11-28" \
        -L \
        -o "$response_file" \
        -s \
        -w "%{http_code}" \
        https://api.github.com/repos/NillionNetwork/devops/branches/master
    ); then
        rm -rf "$response_file"
        echo "Error: Failed to get master branch from GitHub API."
        exit 1
    fi

    if [[ "$http_code" != 2* ]]; then
        rm -rf "$response_file"
        echo "Error: Failed to get 2XX response from GitHub Branches API." >&2
        exit 1
    fi

    local master_sha
    if ! master_sha=$(jq -r '.commit.sha' "$response_file"); then
        rm -rf "$response_file"
        echo "Error: Failed to parse commit SHA from Branches API response." >&2
        exit 1
    fi

    echo "$master_sha"

    rm -rf "$response_file"
}

main() {
    # Validate environment variables.
    if [ -z "$GITHUB_TOKEN" ]; then
        cat <<EOF >&2
Error: GITHUB_TOKEN is empty.

Get the credentials from the jenkins-github-app and set passwordVariable to
GITHUB_TOKEN. Or use a Personal Access Token (PAT).
EOF
        exit 1
    fi

    get_devops_master_sha
}

main "$@"
