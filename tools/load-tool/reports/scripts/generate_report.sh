#!/bin/bash

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}" 2>/dev/null)" && pwd -P)"

if [ $# != 1 ]
then
  echo "Usage: ${0} <input-json-file>"
  exit 1
fi

venv_activate_path="${SCRIPT_DIR}/../venv/bin/activate"
if [ ! -f $venv_activate_path ]
then
  echo "Virtual env is not initialized. Run build_virtualenv.sh first."
  exit 1
fi

input_json_path=$1
input_notebook_path=${SCRIPT_DIR}/../load.ipynb
output_notebook_path=$(echo $input_json_path | sed 's/\.json/\.ipynb/g')
output_html_path=$(echo $input_json_path | sed 's/\.json/\.html/g')

rm -f $output_notebook_path $output_html_path

source $venv_activate_path

papermill $input_notebook_path $output_notebook_path -p results_file_path $input_json_path
jupyter nbconvert --to html $output_notebook_path

echo "HTML report generated in ${output_html_path}"

deactivate
