# shellcheck disable=SC2034
RELEASE_BUCKET_NAME="nillion-releases"

# Creating a tag on the devops repo requires 2 GitHub API calls: one to create
# the tag object and then another to create a ref to the tag.
#
# https://docs.github.com/en/rest/git/tags?apiVersion=2022-11-28#create-a-tag-object
create_devops_tag() {
    local tag="${1:?tag is required by create_devops_tag}"
    local commit="${2:?commit is required by create_devops_tag}"
    local force="${3:-false}"

    if [ "$force" != "false" ] && [ "$force" != "true" ]; then
        echo "Error: force argument has an invalid value '$force'. Must be true or false." >&2
        return 1
    fi

    # Create the tag if trying to fetch an existing one errors out or what's
    # returned is empty/invalid. Otherwise, try to update the tag pointing to a
    # new SHA.
    local tag_sha
    if ! tag_sha=$(get_devops_tag_sha "$tag"); then
        _create_devops_tag "$tag" "$commit"
    elif [ -z "$tag_sha" ]; then
        _create_devops_tag "$tag" "$commit"
    else
        local tag_commit_sha
        if ! tag_commit_sha=$(get_devops_tag_commit_sha "$tag_sha"); then
            echo "Error: Failed to get commit sha for tag '$tag' with sha '$tag_sha'." >&2
            return 1
        fi

        # The GitHub API rejects an update, even with force set to true, if the
        # tag already points at the given commit. Compare the existing and the
        # desired to see if they're the same and return prematurely, without
        # error, if so.
        if [ "$commit" == "$tag_commit_sha" ]; then
            echo "Warning: Tag '$tag' already points to '$commit'. Nothing to do." >&2
            return 2
        fi

        # Update devops tag.
        local response_file
        response_file=$(mktemp)

        if ! http_code=$(curl \
            -H "Accept: application/vnd.github+json" \
            -H "Authorization: Bearer $GITHUB_TOKEN" \
            -H "X-GitHub-Api-Version: 2022-11-28" \
            -X PATCH \
            -d '{"sha":"'"$commit"'","force":'"$force"'}' \
            -o "$response_file" \
            -s \
            -w "%{http_code}" \
            https://api.github.com/repos/NillionNetwork/devops/git/refs/tags/"$tag"
        ); then
            rm -rf "$response_file"
            echo "Error: Failed to update tag ref on devops repo using GitHub Refs API." >&2
            return 1
        fi

        if [[ "$http_code" != 2* ]]; then
            rm -rf "$response_file"
            echo "Error: Got an unexpected HTTP code '$http_code' from GitHub Refs API when updating tag ref." >&2
            return 1
        fi

        rm -rf "$response_file"
    fi
}

_create_devops_tag() {
    local tag="${1:?tag is required by create_devops_tag}"
    local commit="${2:?commit is required by create_devops_tag}"

    local response_file
    response_file=$(mktemp)

    # Create a tag object.
    local http_code
    if ! http_code=$(curl \
        -H "Accept: application/vnd.github+json" \
        -H "Authorization: Bearer $GITHUB_TOKEN" \
        -H "X-GitHub-Api-Version: 2022-11-28" \
        -L \
        -X POST \
        -d '{"tag":"'"$tag"'","message":"","object":"'"$commit"'","type":"commit"}' \
        -o "$response_file" \
        -s \
        -w "%{http_code}" \
        https://api.github.com/repos/NillionNetwork/devops/git/tags
    ); then
        rm -rf "$response_file"
        echo "Error: Failed to create tag on devops repo using GitHub Tags API." >&2
        return 1
    fi

    if [[ "$http_code" != 2* ]]; then
        rm -rf "$response_file"
        echo "Error: Got an unexpected HTTP code '$http_code' from the GitHub Tags API." >&2
        return 1
    fi

    local tag_sha
    if ! tag_sha=$(jq -r '.sha' "$response_file"); then
        rm -rf "$response_file"
        echo "Error: Failed to parse tag SHA from create tag response." >&2
        return 1
    fi

    if [ -z "$tag_sha" ]; then
        rm -rf "$response_file"
        echo "Error: Tag SHA is empty. Something must have gone wrong calling the GitHub Tags API."
        return 1
    fi

    rm -rf "$response_file"

    # Create the tag reference.
    response_file=$(mktemp)

    if ! http_code=$(curl \
        -H "Accept: application/vnd.github+json" \
        -H "Authorization: Bearer $GITHUB_TOKEN" \
        -H "X-GitHub-Api-Version: 2022-11-28" \
        -X POST \
        -d '{"ref":"refs/tags/'"$tag"'","sha":"'"$tag_sha"'","force":'"$force"'}' \
        -o "$response_file" \
        -s \
        -w "%{http_code}" \
        https://api.github.com/repos/NillionNetwork/devops/git/refs
    ); then
        rm -rf "$response_file"
        echo "Error: Failed to create tag ref on devops repo using GitHub Refs API." >&2
        return 1
    fi

    if [[ "$http_code" != 2* ]]; then
        rm -rf "$response_file"
        echo "Error: Got an unexpected HTTP code '$http_code' from GitHub Refs API." >&2
        return 1
    fi

    rm -rf "$response_file"
}

get_devops_tag_sha() {
    local tag="${1:?tag is required by get_devops_tag_sha}"

    # Get tag SHA.
    local response_file
    response_file=$(mktemp)

    local http_code
    if ! http_code=$(curl \
        -H "Accept: application/vnd.github+json" \
        -H "Authorization: Bearer $GITHUB_TOKEN" \
        -H "X-GitHub-Api-Version: 2022-11-28" \
        -o "$response_file" \
        -s \
        -w "%{http_code}" \
        https://api.github.com/repos/NillionNetwork/devops/git/ref/tags/"$tag"
    ); then
        rm -rf "$response_file"
        echo "Error: Failed to get ref for tag on devops repo using GitHub Refs API." >&2
        return 1
    fi

    if [[ "$http_code" != 2* ]]; then
        rm -rf "$response_file"
        echo "Warning: Got a non-2XX HTTP code '$http_code' from GitHub Refs API when looking up tag '$tag'." >&2
        return 1
    fi

    local tag_sha
    if ! tag_sha=$(jq -r '.object.sha' "$response_file"); then
        rm -rf "$response_file"
        echo "Error: Failed to parse tag SHA from get ref response." >&2
        return 1
    fi

    if [ -z "$tag_sha" ]; then
        rm -rf "$response_file"
        echo "Error: Tag SHA is empty. Something must have gone wrong calling the GitHub Refs API." >&2
        return 1
    fi

    rm -rf "$response_file"

    echo "$tag_sha"
}

get_devops_tag_commit_sha() {
    local tag_sha="${1:?tag_sha is required by get_devops_tag_commit_sha}"

    # Now get SHA of commit tag points at.
    response_file=$(mktemp)

    if ! http_code=$(curl \
        -H "Accept: application/vnd.github+json" \
        -H "Authorization: Bearer $GITHUB_TOKEN" \
        -H "X-GitHub-Api-Version: 2022-11-28" \
        -o "$response_file" \
        -s \
        -w "%{http_code}" \
        https://api.github.com/repos/NillionNetwork/devops/git/tags/"$tag_sha"
    ); then
        rm -rf "$response_file"
        echo "Error: Failed to get tag on devops repo using GitHub Tags API." >&2
        return 1
    fi

    if [[ "$http_code" != 2* ]]; then
        rm -rf "$response_file"
        echo "Error: Got an unexpected HTTP code '$http_code' from GitHub Tags API when getting tag." >&2
        return 1
    fi

    local commit_sha
    if ! commit_sha=$(jq -r '.object.sha' "$response_file"); then
        rm -rf "$response_file"
        echo "Error: Failed to parse commit SHA from get tag response." >&2
        return 1
    fi

    if [ -z "$commit_sha" ]; then
        rm -rf "$response_file"
        echo "Error: Commit SHA is empty. Something must have gone wrong calling the GitHub Tags API." >&2
        return 1
    fi

    rm -rf "$response_file"

    echo "$commit_sha"
}

get_release_date() {
    date +"%Y-%m-%d"
}

get_release_commit() {
    git rev-parse --short HEAD
}

get_release_version() {
    local release_date
    release_date=$(get_release_date)

    local release_commit
    release_commit=$(get_release_commit)

    echo "v$release_date-$release_commit"
}
