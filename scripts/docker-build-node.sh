#!/usr/bin/env bash

set -ex

BUILD_PROFILE=$1
TARGET=${2:-x86_64-unknown-linux-musl}

case $TARGET
in
  x86_64-unknown-linux-musl)
    STRIP_CMD=strip
    ;;
  aarch64-unknown-linux-musl)
    STRIP_CMD=aarch64-linux-gnu-strip
    ;;
  *)
    echo "Invalid target ${TARGET}. Accepted values: x86_64-unknown-linux-musl and aarch64-unknown-linux-musl"
    exit 1
    ;;
esac

if [ "$BUILD_PROFILE" != "debug" ] && [ "$BUILD_PROFILE" != "release" ]; then
  echo "Invalid build target ${BUILD_PROFILE}. Accepted values: debug and release"
  exit 1
fi

mkdir -p target/${TARGET}/${BUILD_PROFILE}/docker
cp "target/nillion-release/binaries/${TARGET}/node" target/${TARGET}/${BUILD_PROFILE}/docker/node

$STRIP_CMD target/${TARGET}/${BUILD_PROFILE}/docker/node

scripts/docker-build.sh \
  nillion-node \
  target/${TARGET}/${BUILD_PROFILE}/docker
