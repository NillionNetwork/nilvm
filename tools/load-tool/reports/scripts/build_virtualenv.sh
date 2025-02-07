#!/bin/bash

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}" 2>/dev/null)" && pwd -P)"

pip install --user virtualenv ipykernel
python3 -m venv venv
python3 -m ipykernel install --user --name=venv
source ${SCRIPT_DIR}/../venv/bin/activate
pip install -r ${SCRIPT_DIR}/requirements.txt
deactivate
