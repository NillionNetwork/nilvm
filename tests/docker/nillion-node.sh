#!/bin/bash

# The script checks the health of a nillion-node Docker container.
# It starts a nillion-node container, runs a health check on it, and then removes the container.
#
# Usage:
# ./nillion-node.sh [--no-branch] [--ip IP_ADDRESS]
#
# Options:
# --no-branch    Skips appending the branch name to the container name.
# --ip           Specifies the IP address of the container. If not provided, the IP address will be determined by inspecting running container.
#
# Example:
# ./nillion-node.sh
# ./nillion-node.sh --no-branch
# ./nillion-node.sh --ip 127.0.0.1
# ./nillion-node.sh --no-branch --ip 127.0.0.1

NO_BRANCH=0
IP=""

# This configurations can be set via flags or via env vars
for ((i=1; i<=$#; i++)); do
    case ${!i} in
        --no-branch )
            NO_BRANCH=1
            ;;
        --ip )
            i=$((i+1))
            IP=${!i}
            ;;
    esac
done

SCRIPT_PATH="$(cd "$(dirname "${BASH_SOURCE[0]}" 2>/dev/null)" && pwd -P)"

if [ $NO_BRANCH -eq 0 ]; then
  CLEAN_BRANCH=":$("$SCRIPT_PATH/../../scripts/util_clean_branch_name.sh")"
else
  CLEAN_BRANCH=""
fi

CONTAINER_NAME="nillion-node${CLEAN_BRANCH}"
INSTANCE_NAME="$(scripts/util_clean_docker_run_name.sh node)"

echo "starting nillion-node container test..."

docker run \
  --name "${INSTANCE_NAME}" \
  -v "${SCRIPT_PATH}/../../tests/resources/network/default/1.network.yaml:/network/1.network.yaml" \
  -e "CONFIG_PATH=/network/1.network.yaml" \
  -e "METRICS__LISTEN_ADDRESS=0.0.0.0:34111" \
  -p "34111:34111/tcp" \
  -d "${CONTAINER_NAME}";

if [[ -z "${IP}" ]]; then
  IP=$(docker inspect -f '{{range.NetworkSettings.Networks}}{{.IPAddress}}{{end}}' "${INSTANCE_NAME}")
fi

HEALTHCHECK_ENDPOINT_URL="http://${IP}:34111/metrics"
HEALTHCHECK_REGEX='active_tasks_total\{.*task_type="P2PTransport".*\} 1$'

sleep 2;

curl --max-time 30 -vvv "${HEALTHCHECK_ENDPOINT_URL}"
HEALTHCHECK_OUTPUT=$(curl --max-time 30 -s "${HEALTHCHECK_ENDPOINT_URL}" | grep -E "${HEALTHCHECK_REGEX}")

docker logs "${INSTANCE_NAME}";
docker rm --force "${INSTANCE_NAME}";

if [[ -z "${HEALTHCHECK_OUTPUT}" ]]; then
    echo "===> Nillion docker node health check failed! Didn't find expected output: ${HEALTHCHECK_REGEX}";
    exit 1
else
    echo "===> Nillion docker node health check passed! "
    exit 0
fi
