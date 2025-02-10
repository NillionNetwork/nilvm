#!/usr/bin/env bash

SCRIPT_PATH="$(cd "$(dirname "${BASH_SOURCE[0]}" 2>/dev/null)" && pwd -P)"
if [[ -f "${SCRIPT_PATH}/../.env" ]]; then
  while read entry; do
    [ -n "$entry" ] && export "$(xargs <<< "$entry")"
  done < <(grep -E -v '^#' "${SCRIPT_PATH}/../.env")
fi

if [ "$SCCACHE_ENABLED" != "true" ]; then
  exec "$@"
else
  if [ "$SCCACHE_REGION" = "us" ]
  then
    export SCCACHE_REGION=us-east-1
    export SCCACHE_BUCKET=nilogy-sccache-us
  else
    export SCCACHE_REGION=eu-west-1
    export SCCACHE_BUCKET=nilogy-jenkins-sccache
  fi

  export SCCACHE_S3_USE_SSL=true
  export SCCACHE_ERROR_LOG=/tmp/sccache_log.txt

  sccache $@
fi
