#!/usr/bin/env bash

set -o errexit

SCRIPT_PATH="$(cd "$(dirname "${BASH_SOURCE[0]}" 2>/dev/null)" && pwd -P)"

# shellcheck disable=SC1091
source "$SCRIPT_PATH"/../devops/release-management/scripts/include/release_management.sh

usage() {
    cat <<EOF >&2
Usage: $(basename "$0") [options] tag commit

This program creates a tag on the devops repo using the value of the nillion
repo commit in order to certify that these versions of each repo have been
tested together.

OPTIONS:

    -h, --help,        Show usage.

ARGUMENTS:

    tag,               Tag to create.
    commit,            Commit of tag.

EOF
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

    # Validate args.
    if [ "$#" -eq 0 ]; then
        usage
        exit 1
    fi

    # Parse args.
    local commit
    local tag

    while [ "$#" -gt 0 ]; do
        case "$1" in
            -h|--help)
                usage
                exit
                ;;
            *)
                # Validate positional args.
                if [ "$#" -ne 2 ]; then
                    usage
                    exit 1
                fi

                tag="${1:-}"
                commit="${2:-}"
                shift 2
                ;;
        esac
    done

    if [ -z "$tag" ]; then
        echo "Error: tag is empty." >&2
        exit 1
    fi

    if [ -z "$commit" ]; then
        echo "Error: commit is empty." >&2
        exit 1
    fi

    # Create the tag on the devops repo.
    create_devops_tag "$tag" "$commit"

    echo "Created devops repo tag '$tag' on object '$commit'."
}

main "$@"
