#!/usr/bin/env bash

set -ex

SCRIPT_PATH="$(cd "$(dirname "${BASH_SOURCE[0]}" 2>/dev/null)" && pwd -P)"

$SCRIPT_PATH/install/packages.sh
$SCRIPT_PATH/install/rust.sh
$SCRIPT_PATH/install/tooling.sh
$SCRIPT_PATH/install/uv.sh
$SCRIPT_PATH/install/dmg.sh
