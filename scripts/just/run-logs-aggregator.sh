#!/usr/bin/env bash
# just args: node_logs metrics_log='' wasm_log='' output_path='/tmp/all.json'

usage() {
    cat <<EOF >&2
Usage: $(basename "$0") [options]

This script wraps the log aggregation tool found at 'tools/log-aggregator'
for running with 'just'. The log-aggregator tool holds a README of it's own.

OPTIONS:

    --node-logs files-path,    Space separated list of node log input files (required)
                               e.g.:
                                 --node-logs 'node-1.log node-2.log node-3.log' 

    --output-path file-path,   Path to output file (required)
                               output from 'just run-wasm-logger-server'

    --metrics-log file-path,   Path to metrics log file (optional). Max 1
                               output from 'just run-metrics-scraper-to-file'

    --wasm-log file-path,      Path to wasm log file (optional). Max 1
                               output from 'just run-wasm-logger-server'

    -h|--help,                 Show usage.

EOF
}

main() {
  local node_logs
  local output_path
  local metrics_log
  local wasm_log

  while [ "$#" -gt 0 ]; do
      case "$1" in
          --node-logs)
              node_logs="$2"
              shift 2
              ;;
          --output-path)
              output_path="$2"
              shift 2
              ;;
          --metrics-log)
              metrics_log="$2"
              shift 2
              ;;
          --wasm-log)
              wasm_log="$2"
              shift 2
              ;;
          -h|--help)
              usage
              exit
              ;;
          *)
              echo "left with $1"
              usage
              exit 1
              ;;
      esac
  done

  if [ "$metrics_log" == "x" ]; then
    unset metrics_log;
  fi

  if [ "$wasm_log" == "x" ]; then
    unset wasm_log;
  fi

  echo "===> Running log aggregator"
  echo "+    output_path: ${output_path}"
  
  cmd="python3 tools/log-aggregator/merge.py --output ${output_path}"
  for node_log in ${node_logs}; do
    echo "+    node_log: $node_log"
    cmd="$cmd --node $node_log"
  done
  
  if [ -n "${metrics_log}" ]; then
    echo "+    metrics_log: ${metrics_log}"
    cmd="$cmd --metrics ${metrics_log}"
  else
    echo "!!   metrics_log: (unset)"
  fi
  
  if [ -n "${wasm_log}" ]; then
    echo "+    wasm_log: ${wasm_log}"
    cmd="$cmd --wasm ${wasm_log}"
  else
    echo "!!   wasm_log: (unset)"
  fi
  
  echo ""
  echo "=    ($cmd)"
  exec $cmd
}

main "$@"
