#!/bin/bash

BUILD_TYPE="${BUILD_TYPE:-debug}"
SCRIPT_PATH="$( cd -- "$(dirname "$0")" >/dev/null 2>&1; pwd -P )"
ROOT_PATH=$SCRIPT_PATH/..
NODE_BIN_PATH=$ROOT_PATH/target/$BUILD_TYPE/node
BASE_OUTPUT_PATH=/tmp/nillion-network
NODE_CONFIG_DIR_PATH=$ROOT_PATH/tests/resources/network/testing

echo "===> Using $BUILD_TYPE node in $NODE_BIN_PATH"
pids=()
for i in $(seq 1 5)
do

  if [ $i -eq 5 ]; then
    echo "We will start 5th node after for 30 secs"
    sleep 30
  fi

  config_path="${NODE_CONFIG_DIR_PATH}/${i}.network.yaml"
  echo "Node config file: ${output_path}"

  output_path=$BASE_OUTPUT_PATH/node_$i
  echo "Node output path: ${output_path}"

  rm "$output_path/*"
  mkdir -p "$output_path"
  echo "Starting node $i using config file $config_path"
  CONFIG_PATH=$config_path RUST_LOG=DEBUG $NODE_BIN_PATH >"$output_path/log" 2>"$output_path/err" &

  pid=$!
  pids+=($pid)
done

for pid in ${pids[@]}
do
   wait $pid
done
