#!/usr/bin/env bash

if [[ "${BASH_SOURCE[0]}" == "" ]]; then
    echo "it appears that you've 'sourced' this script; but you should execute it instead"
    echo "./setup_env.sh"
    return
fi

SCRIPT_PATH="$( cd "$(dirname ${BASH_SOURCE[0]} || dirname ${(%):-%N})" && pwd -P )"

git config core.hooksPath ${SCRIPT_PATH}/.githooks

bash ${SCRIPT_PATH}/scripts/install_all.sh

echo -e "\n\n"
echo "Finished setting up environment"
echo ""
