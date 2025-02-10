#!/bin/bash

VENV_NAME="loadtest-venv"
REQUIREMENTS_FILE="requirements.txt"

if ! pip show virtualenv > /dev/null; then
    echo "virtualenv is not installed. Installing now..."
    pip install virtualenv
fi

virtualenv $VENV_NAME

source ./$VENV_NAME/bin/activate

if [ -f "$REQUIREMENTS_FILE" ]; then
    echo "Installing requirements from $REQUIREMENTS_FILE"
    pip install -r $REQUIREMENTS_FILE
else
    echo "No requirements.txt found at $REQUIREMENTS_FILE"
fi

echo "Virtual environment '$VENV_NAME' is set up and activated."
echo "To deactivate the virtual environment, run 'deactivate' command."
