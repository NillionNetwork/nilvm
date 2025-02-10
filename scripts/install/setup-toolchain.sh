#!/usr/bin/env bash
set -e

SCRIPT_PATH="$(cd "$(dirname "${BASH_SOURCE[0]}" 2>/dev/null)" && pwd -P)"

function setup-linux-musl-gcc-toolchain {
  TARGET=$1
  MUSL_GCC_TARGET=$(translate-target-to-musl-gcc-target "$TARGET")
  INSTALL_PATH=$2
  cd "$INSTALL_PATH"
  [[ -d "toolchains/linux/${TARGET}" ]] && rm -rf "toolchains/linux/${TARGET}"
  mkdir -p "toolchains/linux/"
  cd toolchains/linux
  curl "http://more.musl.cc/10/x86_64-linux-musl/$MUSL_GCC_TARGET-cross.tgz" -o "$MUSL_GCC_TARGET-cross.tgz"
  tar -xvf "$MUSL_GCC_TARGET-cross.tgz"
  rm "$MUSL_GCC_TARGET-cross.tgz"
  mv "$MUSL_GCC_TARGET-cross" "$TARGET"
  cd "$TARGET/bin"

  ls | while read file; do
    new_file=$(echo $file | sed -e "s|$MUSL_GCC_TARGET|$TARGET|g")
    mv "$file" "$new_file"
  done

  rm "$TARGET-cc"
  ln -s "$TARGET-gcc" "$TARGET-cc"
}

function translate-target-to-musl-gcc-target {
  case $1 in
  x86_64-unknown-linux-musl)
    echo x86_64-linux-musl
    ;;
  aarch64-unknown-linux-musl)
    echo aarch64-linux-musl
    ;;
  *)
    echo "Unknown target '$1'"
    exit 1
    ;;
  esac
}

TARGET="${1:?"target not provided (i386-apple-darwin11, x86_64-apple-darwin11, arm-apple-darwin11 ...)"}"
INSTALL_PATH="${2:?"install path not provided"}"

if [[ "$TARGET" == *linux* ]]; then
  setup-linux-musl-gcc-toolchain "$TARGET" "$INSTALL_PATH"
else
  echo "Unsupported target '$TARGET'"
  exit 1
fi
