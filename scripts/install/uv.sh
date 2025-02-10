#!/bin/bash

set -euo pipefail

PYTHON_VERSION=3.10
UV_VERSION=0.5.7
INSTALL_DIR="${HOME}/.local/bin"

curl --proto '=https' --tlsv1.2 -LsSf https://github.com/astral-sh/uv/releases/download/0.5.7/uv-installer.sh | sh

export PATH="${INSTALL_DIR}:$PATH"

uv python install "${PYTHON_VERSION}"
