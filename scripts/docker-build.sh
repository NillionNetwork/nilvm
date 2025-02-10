#!/usr/bin/env bash

set -e

NAME=$1
shift
CONTEXT=$1
shift
DOCKERFILE="docker/${NAME}.dockerfile"

# This configurations can be set via flags or via env vars
while [ $# -gt 0 ]; do
    case $1 in
        --expose-aws-creds )
            EXPOSE_AWS_CRED="true"
            ;;
        --expose-ssh-creds )
            EXPOSE_SSH_CRED="true"
            ;;
        --with-cache )
            WITH_CACHE="true"
            ;;
        --remote-cache )
            USE_REMOTE_CACHE="true"
            ;;
        --dockerfile )
            DOCKERFILE=$2
            shift
            ;;
    esac
    shift
done

SCRIPT_PATH="$(cd "$(dirname "${BASH_SOURCE[0]}" 2>/dev/null)" && pwd -P)"
# Load env vars
[[ -f "${SCRIPT_PATH}/../.env" ]] && export $(grep -E -v '^#' "${SCRIPT_PATH}/../.env" | xargs)

CLEAN_BRANCH="$("$SCRIPT_PATH/util_clean_branch_name.sh")"
COMMIT_SHA=$("$SCRIPT_PATH"/get-commit-sha.sh)

function expose_aws_creds {
  if [[ "$EXPOSE_AWS_CRED" == "true" ]]
  then
    echo '--build-arg AWS_ACCOUNT_ID
    --build-arg AWS_DEFAULT_REGION
    --build-arg AWS_ACCESS_KEY_ID
    --build-arg AWS_SECRET_ACCESS_KEY'
  fi
}

function with_cache {
  if [[ "$WITH_CACHE" != "true" ]]
  then
    echo "--no-cache"
  fi
}

function expose_ssh_creds {
  if [[ "$EXPOSE_SSH_CRED" == "true" ]]
  then
    echo "--ssh default"
  fi
}

function with_docker_build_cache {
  CONTAINER_NAME="$1"
  VERSION="$2"
  if [[ "$USE_REMOTE_CACHE" != "true" ]]; then
      # do nothing
      true
  elif aws ecr describe-images --repository-name="${CONTAINER_NAME}" --image-ids="imageTag=${VERSION}" >/dev/null 2>&1; then
      echo "--cache-from ${AWS_ACCOUNT_ID}.dkr.ecr.${AWS_DEFAULT_REGION}.amazonaws.com/${CONTAINER_NAME}:${VERSION}"
  fi
}

function pull_image_for_build_cache {
  CONTAINER_NAME="$1"
  VERSION="$2"
  if [[ "$USE_REMOTE_CACHE" != "true" ]]; then
      # do nothing
      true
  elif aws ecr describe-images --repository-name="${CONTAINER_NAME}" --image-ids="imageTag=${VERSION}" > /dev/null 2>&1; then
      docker pull "${AWS_ACCOUNT_ID}.dkr.ecr.${AWS_DEFAULT_REGION}.amazonaws.com/${CONTAINER_NAME}:${VERSION}"
  fi
}

export DOCKER_BUILDKIT=1 # Use build kit to be able to use build time secrets

if [[ "$USE_REMOTE_CACHE" == "true" ]]
then
    CONTAINER_NAME="$NAME" "$SCRIPT_PATH/docker-login.sh"
    pull_image_for_build_cache "$NAME" "$COMMIT_SHA"
    pull_image_for_build_cache "$NAME" latest
fi

docker build \
  --label "com.nillion.commit-sha=$COMMIT_SHA" \
  --label "com.nillion.git-log=$(git log --oneline --all | head -n 25)" \
  --label "com.nillion.commit-date=$(git show --no-patch --no-notes --pretty='%cd' HEAD)" \
  --label "com.nillion.build-date=$(date -u +'%Y-%m-%dT%H:%M:%SZ')" \
  --label "com.nillion.build-number=${BUILD_NUMBER:-local}" \
  --label "com.nillion.application-name=${NAME}" \
  --build-arg BUILDKIT_INLINE_CACHE=1 \
  $(expose_aws_creds) \
  $(with_cache) \
  $(expose_ssh_creds) \
  $(with_docker_build_cache "$NAME" "$COMMIT_SHA") \
  $(with_docker_build_cache "$NAME" "latest") \
  -t "${NAME}:${CLEAN_BRANCH}" \
  -f "${DOCKERFILE}" \
  "${CONTEXT}"

docker tag "${NAME}:${CLEAN_BRANCH}" "${NAME}:latest"
