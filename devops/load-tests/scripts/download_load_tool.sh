#!/usr/bin/env bash

PRIVATE_TOOLS=nillion-private-tools-x86_64-unknown-linux-musl.tar.gz

DOWNLOAD_PATH="${1:?DOWNLOAD_PATH is required}"
RELEASE_VERSION="${2:?RELEASE_VERSION is required}"

aws s3 cp --no-progress "s3://nillion-private-releases/$RELEASE_VERSION/$PRIVATE_TOOLS" "$DOWNLOAD_PATH"
tar xfvz "$DOWNLOAD_PATH/$PRIVATE_TOOLS" -C "$DOWNLOAD_PATH"
