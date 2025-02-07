#!/usr/bin/env bash
set -e
set -o pipefail

MY_REPO='git@github.com:NillionNetwork/nillion'

KNOWN_HOSTS_FILE="$HOME/.ssh/known_hosts"
GITHUB_DOMAIN="github.com"

if [ ! -d "$HOME/.ssh" ]; then
    mkdir -p "$HOME/.ssh"
    chmod 700 "$HOME/.ssh"
fi

if ! grep -q "$GITHUB_DOMAIN" "$KNOWN_HOSTS_FILE"; then
    ssh-keyscan -t rsa "$GITHUB_DOMAIN" >> "$KNOWN_HOSTS_FILE"
fi

# jenkins clones are https origins but we have ssh creds in the jenkins context
# so when we atttempt to compare branches in bors (is a shallow clone) against 
# main it doesn't work. here we add a ssh based remote
if ! git remote --verbose | grep --quiet "$MY_REPO"; then
  git remote add -t main compare git@github.com:NillionNetwork/nillion
fi

MY_REMOTE=$(git remote --verbose | grep "$MY_REPO" | awk '{print $1}' | tail -n 1)
git fetch "$MY_REMOTE" main HEAD > /dev/null 2>&1
HEAD_MAIN_SHA=$(git merge-base HEAD "$MY_REMOTE/main")
git diff --name-only "$HEAD_MAIN_SHA" HEAD | grep -E ".*\.sh$" || true | xargs --verbose -I {} shellcheck --external-sources --shell=bash {}
