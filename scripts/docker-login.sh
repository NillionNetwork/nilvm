#!/bin/bash

SCRIPT_PATH="$(cd "$(dirname "${BASH_SOURCE[0]}" 2>/dev/null)" && pwd -P)"
# Load env vars
[[ -f "${SCRIPT_PATH}/../.env" ]] && export $(grep -E -v '^#' "${SCRIPT_PATH}/../.env" | xargs)

PUBLIC=${1}

if [[ "$PUBLIC" == "--public" ]]
then
  REPOSITORY_URL="public.ecr.aws"
  AWS_DEFAULT_REGION="us-east-1"
else
  REPOSITORY_URL="${AWS_ACCOUNT_ID}.dkr.ecr.${AWS_DEFAULT_REGION}.amazonaws.com"
fi

REPOSITORY_URL="${AWS_ACCOUNT_ID}.dkr.ecr.${AWS_DEFAULT_REGION}.amazonaws.com"
AWS_CLI_MAJOR_VERSION=$(aws --version 2>&1 | cut -d " " -f1 | cut -d "/" -f2 | cut -d "." -f1)

# Login to aws registry
if [[ $AWS_CLI_MAJOR_VERSION -eq "1" ]]
then
  # shellcheck disable=2091
  $(aws ecr get-login --no-include-email --region "${AWS_DEFAULT_REGION}")
elif [[ $AWS_CLI_MAJOR_VERSION -eq "2" ]]
then
  aws ecr get-login-password --region "${AWS_DEFAULT_REGION}" | docker login -u AWS --password-stdin "${REPOSITORY_URL}"
else
  echo "Unknown AWS CLI version: ${AWS_CLI_MAJOR_VERSION}"
  exit 1
fi
