#!/usr/bin/env bash

set -ex

# debian base packages install
if [ -f "/etc/debian_version" ]; then
  # Install required packages on Debian-based distros
  echo "Installing required packages"
  sudo apt install -y build-essential libssl-dev pkg-config protobuf-compiler lld musl-tools \
    libpython3.10-dev make cmake git curl wget hfsprogs \
    zlib1g-dev libbz2-dev libreadline-dev libsqlite3-dev \
    libncursesw5-dev xz-utils tk-dev libxml2-dev libxmlsec1-dev libffi-dev liblzma-dev \
    shellcheck
fi


# macOS stuff
if [[ $OSTYPE == "darwin"* ]]; then
  # Install musl-based GCC macOS-to-Linux cross-compiler
  brew install filosottile/musl-cross/musl-cross openssl readline sqlite3 xz zlib tcl-tk \
    shellcheck cmake llvm awscli jq protobuf chromedriver pidof git curl
fi
