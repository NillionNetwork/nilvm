#!/bin/bash

ECR_SUBCOMMAND="ecr"

if [[ "$1" == "--public" ]]
then
  shift
  PUBLIC="true"
  PUBLIC_LOGIN_FLAG="--public"
  PUBLIC_ORG_NAME="$1"
  ECR_SUBCOMMAND="ecr-public"
  shift
fi

CONTAINER_NAME="$1"
TAG="$2" # Optional, if there is a tag only that tag is pushed

SCRIPT_PATH="$(cd "$(dirname "${BASH_SOURCE[0]}" 2>/dev/null)" && pwd -P)"

if [[ $PUBLIC == "true" ]]
then
  CONTAINER_URI="public.ecr.aws/${PUBLIC_ORG_NAME}/${CONTAINER_NAME}"
else
  CONTAINER_URI="${AWS_ACCOUNT_ID}.dkr.ecr.${AWS_DEFAULT_REGION}.amazonaws.com/${CONTAINER_NAME}"
fi

CLEAN_BRANCH="$("$SCRIPT_PATH/util_clean_branch_name.sh")"
SOURCE_CONTAINER_NAME="${CONTAINER_NAME}:${CLEAN_BRANCH}"


SELF_VERSION=$(
  docker inspect ${SOURCE_CONTAINER_NAME} --format "{{range .Config.Env}}{{println .}}{{end}}" \
  | grep "SELF_VERSION" \
  | sed -e "s|SELF_VERSION=\(.*\)|\1|g"
  )

COMMIT_SHA=$("$SCRIPT_PATH"/get-commit-sha.sh)

# Load env vars
[[ -f "${SCRIPT_PATH}/../.env" ]] && export $(grep -E -v '^#' "${SCRIPT_PATH}/../.env" | xargs)

if [[ $PUBLIC == "true" ]]
then
  export AWS_DEFAULT_REGION="us-east-1"
  aws sts
fi
# Support in this script for auto-creation of ECR repositories has been removed.
# Here we check if the ECR repository exists. If it does, then the script
# proceeds. If it doesn't, engineers are directed to create the repo elsewhere.
if ! res=$(aws $ECR_SUBCOMMAND describe-repositories --repository-names "$CONTAINER_NAME" 2>&1); then
    if echo "$res" | grep "RepositoryNotFoundException" >/dev/null; then
        echo -e "\033[1;31mSorry. ECR repository '$CONTAINER_NAME' does not exist. Please create the repository in the devops repo under the baseline deployment and root plan.\033[0m"
    else
        echo -e "\033[1;31mFailed with unexpected AWS error when checking if ECR repository '$CONTAINER_NAME' exists:\033[0m"
        echo -e "\033[1;31m$res\033[0m"
    fi

    exit 1
fi

"$SCRIPT_PATH/docker-login.sh" "$PUBLIC_LOGIN_FLAG"

function check_override_push {
  local VERSION=$1
  if aws $ECR_SUBCOMMAND describe-images --repository-name="${CONTAINER_NAME}" --image-ids="imageTag=${VERSION}" > /dev/null 2>&1; then
    echo -e "\033[1;31mAn image with version ${CONTAINER_NAME}:${VERSION} already exist, are you sure you want to override it?\033[0m"
    echo -e "\033[1;31mThis could affect all deployments\033[0m"
    if [ "$DOCKER_PUBLISH_AUTO_YES" == "" ]; then
      echo -e "\033[1;31m[yN]\033[0m"
      read -r answer
      if [[ $answer != 'y' ]]; then
        return 0
      fi
    fi

    local OVERRIDDEN_VERSION="${VERSION}-overridden-on-$(date -u +'%Y-%m-%dT%H-%M-%SZ')"
    echo -e "Tagging overridden version as \033[1;31m$OVERRIDDEN_VERSION\033[0m"
    local MANIFEST=$(aws $ECR_SUBCOMMAND batch-get-image --repository-name="${CONTAINER_NAME}" --image-ids="imageTag=${VERSION}" --query 'images[].imageManifest' --output text)
    aws $ECR_SUBCOMMAND put-image --repository-name="${CONTAINER_NAME}" --image-tag "${OVERRIDDEN_VERSION}" --image-manifest "$MANIFEST"
  fi

  docker push "${CONTAINER_URI}:${VERSION}"

}

# Tag and Push Container
if [ "${TAG}" != "" ]; then
    docker tag "${SOURCE_CONTAINER_NAME}" "${CONTAINER_URI}:${TAG}"
    check_override_push "${TAG}"
else
  if [ "${BRANCH_NAME}" = "main" ]; then
      docker tag "${SOURCE_CONTAINER_NAME}" "${CONTAINER_URI}:latest"
      docker push "${AWS_ACCOUNT_ID}.dkr.ecr.${CONTAINER_URI}:latest"
  fi

  if [ "$SELF_VERSION" != "" ]; then
      docker tag "${SOURCE_CONTAINER_NAME}" "${CONTAINER_URI}:${SELF_VERSION}"
      check_override_push "${SELF_VERSION}"
  fi

  docker tag "${SOURCE_CONTAINER_NAME}" "${CONTAINER_URI}:$COMMIT_SHA"
  docker tag "${SOURCE_CONTAINER_NAME}" "${CONTAINER_URI}:${CLEAN_BRANCH}-${BUILD_NUMBER:-local}"
  docker tag "${SOURCE_CONTAINER_NAME}" "${CONTAINER_URI}:${CLEAN_BRANCH}"
  check_override_push "$COMMIT_SHA"
  check_override_push "${CLEAN_BRANCH}-${BUILD_NUMBER:-local}"
  check_override_push "${CLEAN_BRANCH}"
fi
