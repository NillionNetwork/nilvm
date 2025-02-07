#!/bin/bash

pip install --user virtualenv ipykernel
python3 -m venv venv
python3 -m ipykernel install --user --name=venv
source venv/bin/activate
pip install -r requirements.txt
deactivate
