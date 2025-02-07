#!/usr/bin/env bash

set -o errexit

PLATFORM="${1:?PLATFORM is required}"
DOWNLOAD_PATH="${2:?DOWNLOAD_PATH is required}"
RELEASE_VERSION="${3:?RELEASE_VERSION is required}"
FORCE="${4:?false}"
UNPACK="${5:?true}"

if [[ "$FORCE" == "true" ]]; then
    rm -rf "$DOWNLOAD_PATH"
fi

aws s3 cp --no-progress --recursive s3://nillion-releases/"$RELEASE_VERSION"/ "$DOWNLOAD_PATH"

if [[ "$UNPACK" == "true" ]]; then
    tar xfvz "$DOWNLOAD_PATH/nillion-sdk-bins-$PLATFORM.tar.gz" -C "$DOWNLOAD_PATH"
fi


echo "SDK '$RELEASE_VERSION' downloaded to $DOWNLOAD_PATH."
