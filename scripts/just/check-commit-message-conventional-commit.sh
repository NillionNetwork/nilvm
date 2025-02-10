#!/usr/bin/env bash
EXIT_CODE=0
git fetch origin main > /dev/null 2>&1
git log --pretty=format:%s origin/main..HEAD | while read line
do
    if ! echo $line | grep -E '^(\bbuild\b|\bchore\b|\bci\b|\bdocs\b|\bfeat\b|\bfix\b|\bperf\b|\brefactor\b|\brevert\b|\bstyle\b|\btest\b)(\([a-z ]+\))!?: [a-zA-Z -]*$' > /dev/null
    then
        echo $line
        echo "
Your commit message doesn't conform to Convential Commit Rules (https://www.conventionalcommits.org/).
At a minimum, your commit should start with one of the below keywords:

build: chore: ci: docs: feat: fix: perf: refactor: revert: style: test:
      "
        EXIT_CODE=1
    fi
done
exit $EXIT_CODE
