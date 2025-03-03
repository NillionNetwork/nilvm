#!/usr/bin/env bash

set -o errexit

TAG="${1:?TAG is not set}"
COMMIT="${2:?COMMIT is not set}"
GIT_USER_EMAIL="${3:-jenkins@nillion.com}"
GIT_USER_NAME="${4:-nillion-jenkins}"
FORCE="${5:-false}"

# Force-push tag if set to true.
FORCE_OPTS=""
if [ "$FORCE" == "true" ]; then
    FORCE_OPTS="-f"
fi

# Set git configs in case they have not been previously set, which is the case with GitHub Actions,
# but not with Jenkins where we typically surround the script with
# `withJenkinsGitHubSSHCredentials`.
if ! git config --global user.email; then
    git config --global user.email "$GIT_USER_EMAIL"
fi

if ! git config --global user.name; then
    git config --global user.name "$GIT_USER_NAME"
fi

# Use "^{}" to ensure we tag the object being pointed to, if e.g. $COMMIT is
# actually a tag itself.
git tag -a "$TAG" $FORCE_OPTS -m "$TAG" "$COMMIT^{}"

# Avoid host key verification errors by implicitly trusting the git config.
export GIT_SSH_COMMAND="/usr/bin/ssh -o StrictHostKeyChecking=no"
git push $FORCE_OPTS origin "$TAG"
