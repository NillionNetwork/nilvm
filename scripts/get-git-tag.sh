#!/usr/bin/env bash

set -o errexit

REV="${1:?REV is not set}"

git describe --tags --abbrev=0 origin/"$REV"
