#!/bin/bash

set -e

if [ $# -eq 0 ]
then
  echo "Usage: $0 release_version artifact1 [artifact2 [...]]"
  exit 1
fi

SCRIPT_PATH="$(cd "$(dirname "${BASH_SOURCE[0]}" 2>/dev/null)" && pwd -P)"

# shellcheck disable=SC1091
source "$SCRIPT_PATH"/include/release_management.sh

RELEASE_VERSION="$1"
shift

echo "Using version ${RELEASE_VERSION}"

VERSION_FILE="$(mktemp)"
echo "$RELEASE_VERSION" >"$VERSION_FILE"
aws s3 cp "$VERSION_FILE" "s3://${RELEASE_BUCKET_NAME}/${RELEASE_VERSION}/RELEASE.md"

# shellcheck disable=SC2068
for artifact in $@
do
  echo "Uploading ${artifact}"
  filename=$(basename "$artifact")
  aws s3 cp "$artifact" "s3://${RELEASE_BUCKET_NAME}/${RELEASE_VERSION}/${filename}"
done

