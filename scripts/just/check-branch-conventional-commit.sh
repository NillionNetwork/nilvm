#!/usr/bin/env bash
# in Jenkins git rev-parse --abbrev-ref HEAD will return HEAD, so we look at CHANGE_BRANCH env var set by jenkins instead
BRANCH_NAME=$(
if [[ "$CHANGE_BRANCH" != "" ]]
then
    echo "$CHANGE_BRANCH"
else
    git rev-parse --abbrev-ref HEAD
fi
)
echo $BRANCH_NAME | if ! grep -E --quiet "^\bdependabot\b|\bbuild\b|\bchore\b|\bci\b|\bdocs\b|\bfeat\b|\bfix\b|\bperf\b|\brefactor\b|\brevert\b|\bstyle\b|\btest\b(\([a-z ]+\)!)?/[a-zA-Z0-9\_-]+$"
then
    echo $BRANCH_NAME
    echo "
Your branch name doesn't conform to Convential Commit Rules (https://www.conventionalcommits.org/).
Your branch should start with one of the below keywords:

build/ chore/ ci/ docs/ feat/ fix/ perf/ refactor/ revert/ style/ test/
    "
    exit 1
fi