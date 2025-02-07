#!/usr/bin/env zsh
# shellcheck disable=SC1091

set -o errexit

SCRIPT_PATH="$(realpath "$(dirname "$0")")"

DOWNLOAD_PATH="${1:?DOWNLOAD_PATH is required}"
ENV_NAME="${2:?ENV_NAME is required}"
REPORT_PATH="${3:?REPORT_PATH is required}"
SPEC_PATH="${4:?SPEC_PATH is required}"
VERBOSE="${5:?VERBOSE is required}"

: "${PRIVATE_KEY:?is required and expected to be in environment}"

# Constants.
DEPLOYMENT="nillion-network"
GITHUB_DEVOPS_DIR="$SCRIPT_PATH/../../github-devops"
PLATFORM="x86_64-unknown-linux-musl"

if [ ! -d "$GITHUB_DEVOPS_DIR" ]; then
  echo "devops repo has not been cloned to '$GITHUB_DEVOPS_DIR'. Code-sharing is not possible. Use the clone-devops-repo just-target before proceeding." >&2
  exit 1
fi

source "$GITHUB_DEVOPS_DIR"/pipelines/shared/shell/ops.sh
source "$GITHUB_DEVOPS_DIR"/pipelines/shared/shell/ssh.sh
source "$GITHUB_DEVOPS_DIR"/lib/shell/terraform.sh
source "$GITHUB_DEVOPS_DIR"/pipelines/shared/shell/tunnel.sh
source "$GITHUB_DEVOPS_DIR"/pipelines/shared/shell/util.sh

echo "Fetching terraform variables"
terraform_eval_or_default "$GITHUB_DEVOPS_DIR" "$DEPLOYMENT" "$ENV_NAME" "nodes" "BOOTNODE_GRPC_ENDPOINT" "NILCHAIN_RPC_URL"
nonempty_or_exit "Terraform output 'bootnode_grpc_endpoint' is empty." "$BOOTNODE_GRPC_ENDPOINT"
nonempty_or_exit "Terraform output 'nilchain_rpc_url' is empty." "$NILCHAIN_RPC_URL"

# Determine optional flags for load-tool.
if [[ "$VERBOSE" == "true" ]]; then
  OPTS="--verbose"
else
  OPTS=
fi

echo "Invoking load tool"
# Run load-tool.
# shellcheck disable=SC2086
"$DOWNLOAD_PATH"/"$PLATFORM"/load-tool \
  --bootnode "$BOOTNODE_GRPC_ENDPOINT" \
  --nilchain-rpc-endpoint "$NILCHAIN_RPC_URL" \
  --nilchain-stash-private-key "$PRIVATE_KEY" \
  --output-path "$REPORT_PATH" \
  --spec-path "$SPEC_PATH" $OPTS
