#!/usr/bin/env bash

# The primary purpose of this script is to encapsulate the manner in which the
# commit SHA is resolved for scripts/docker-build.sh, scripts/docker-publish.sh
# and the Justfile target used by the multi-branch pipeline in order to trigger
# the nillion-network/deploy job instead of having "git rev-parse HEAD"
# dispersed over each of those files.
git rev-parse HEAD
