#!/usr/bin/env bash

set -o errexit

SCRIPT_PATH="$(cd "$(dirname "${BASH_SOURCE[0]}" 2>/dev/null)" && pwd -P)"

# shellcheck disable=SC1091
source "$SCRIPT_PATH"/include/release_management.sh

usage() {
    cat <<EOF >&2
Usage: $(basename "$0") [options] new_tag existing_tag force

This program retags an existing tag.

OPTIONS:

    -h, --help,        Show usage.

ARGUMENTS:

    new_tag,           Tag to create.
    existing_tag,      Existing tag that new tag will point at.
    force,             Force tag creation.

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
    local new_tag
    local existing_tag

    while [ "$#" -gt 0 ]; do
        case "$1" in
            -h|--help)
                usage
                exit
                ;;
            *)
                # Validate positional args.
                if [ "$#" -ne 3 ]; then
                    usage
                    exit 1
                fi

                new_tag="${1:-}"
                existing_tag="${2:-}"
                force="${3:-}"
                shift 3
                ;;
        esac
    done

    # Get commit to which existing tag points.
    local tag_sha
    if ! tag_sha=$(get_devops_tag_sha "$existing_tag"); then
        echo "Error: Failed to get tag sha for tag '$existing_tag'." >&2
        exit 1
    fi

    local tag_commit_sha
    if ! tag_commit_sha=$(get_devops_tag_commit_sha "$tag_sha"); then
        echo "Error: Failed to get commit sha for tag '$existing_tag'." >&2
        exit 1
    fi

    if ! create_devops_tag "$new_tag" "$tag_commit_sha" "$force"; then
        local exit_code=$?

        # Exit code 2 is returned when the tag already points at the correct
        # commit. In this case, this is not an error condition.
        if [ $exit_code -ne 2 ]; then
            exit $exit_code
        fi
    else
        echo "Created devops repo tag '$new_tag' based on existing tag '$existing_tag' which points to object '$tag_commit_sha'." >&2
    fi
}

main "$@"
