#!/bin/bash

source "$(git rev-parse --show-toplevel || echo .)/scripts/activate_venv.sh" venv
pip3 install -r scripts/jenkins-build-timing-report.requirements.txt
python3 scripts/jenkins-build-timing-report.py "$@"
