# Description

This is a set of tools to help analyze the Nillion network logs using an open source logging viewer called QLogExplorer

It pulls together nillion node stderr logs, nillion node prometheus metrics and wasm log output and combines them in
sequence by timestamp so that you can view behavior end-to-end

This is tested using a single node with full DEBUG loglevel.

QLogExplorer is nice because we can share templates so that we can make rules to slice n' dice what is shown and how,
apply highlight rules, and the like.


# Prereqs

## Installing QLogExplorer

See https://rafaelfassi.github.io/qlogexplorer/

This was available in my Ubuntu 22.04 repo (Pop!_OS) but looks to have a pretty standard build procedure.


## Adding QLogExplorer templates

After starting QLogExplorer for the first time, it will populate config paths. Exit the program and then:

1. Start the QLogExplorer program, then exit (so that it creates a default config structure on disk)
2. Copy templates from this project to the well known config location
   Use this just command top copy templates: `just run-init-qlogexplorer-templates`


# Usage

First thing to do is gather your log files. There will be three sets.

1. Kick off network and generate your node logs. These are generated/saved when running our five node system and will deposit them
   on your workspace: `tests/run_local_network/logs/stderr`
   Start your network using this just command: `just run-local-network`
2. Start websocket log reciever.
   Start your reciever using this just command: `just run-wasm-logger-server <YOUR-DESIRED-OUTPUT-DIR>`
3. Start the metrics scraper (see below for alt options)
   Start your scraper using this just command: `just run-metrics-scraper-to-file <TARGET-NODE> <METRIC-SUBSTRING> <OUTPUT-PATH>`
4. Run your test scenarios so that logs are fully populated
5. Stop your logging stack
6. Combine logs using tool.
   Exec your combiner with this just command: `just run-logs-aggregator <NODE-LOG-PATH> <WASM-OUTPUT-PATH> <METRIC-OUTPUT-PATH> <COMBINED-OUTPUT-PATH>`
7. Open QLogExplorer - File > Open As > Nillion all
   Browse to your <COMBINED-OUTPUT-PATH>


# Example

```shell
# shell 1
# optionally run network
# just run-local-network
```

```shell
# shell 2
# optional: this is for live js-client log collection (eg: niltransfer)
just run-wasm-logger-server /tmp/wasm.out 11100
```

```shell
# shell 3
# optionally pipe to remote machine!
# ssh -L 34111:localhost:34111 nillion-testnet-prod-5 -N &
just run-metrics-scraper-to-file localhost:34111 preprocessing_generated_elements_total /tmp/metrics-out.json

```

> When log collection is completed...

```shell
# shell 4
just run-logs-aggregator \
    tests/run_local_network/logs/stderr/nvm-node-127.0.0.1:24111.stderr.log \  # node log path       (required)
    /tmp/wasm-out/default.log \                                                # wasm log path       (optional)
    /tmp/metrics-out.json \                                                    # metrics scrape path (optional)
    /tmp/all.json                                                              # output for viewer   (default: /tmp/all.json)
```

# NOTES

1. wasm or metrics logs can be ommitted
2. You can include multiple node log files by providing a space separated string like this:
```shell
just run-logs-aggregator 'node-1.log node-2.log node-3.log' 
```
> the above command will merge all 3 node log files into `/tmp/all.json`
3. You can fetch a remote node log like this
```shell
ssh nillion-testnet-prod-5 'sudo cp -v $(docker inspect node --format=''{{.LogPath}}'') /tmp/node-5.out; sudo chmod 777 /tmp/node-5.out'
scp nillion-testnet-prod-5:/tmp/node-5.out /tmp/
```
4. ... or you can pipe out the log output like this
```shell
ssh nillion-testnet-prod-5 'docker logs node' >/tmp/node-5.out'

```


# Reference

## Dumping prometheus metrics to a file

usage: node-metrics-log-generator.py [-h] -t TARGET -m METRIC -o OUTPUT [-w WAIT]

Gathers metrics and writes to file for use in QLogExplorer

optional arguments:
  -h, --help            show this help message and exit
  -t TARGET, --target TARGET
                        One or more node host:port
  -m METRIC, --metric METRIC
                        One or more prometheus metric substrings to capture
  -o OUTPUT, --output OUTPUT
                        One JSON struct per-line of output filename
  -w WAIT, --wait WAIT  How many seconds to wait between probes (default: 1)
