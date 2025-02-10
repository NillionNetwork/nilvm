# nillion-devnet

This tool allows running a local Nillion cluster.

## Usage

Simply run the command to start a local cluster. A single node will be selected to be a bootnode, and its address will
be printed after booting the cluster up:

```shell
$ nillion-devnet
ℹ️cluster id is c6ed53de-465e-48a5-b55a-60110e82db97
ℹ️storing state in /tmp/.tmpZ0gWEM
🏃starting node 12D3KooWLKKmDNUtuhD56qCmwecBmeN2q5QzhuQAuWekVffASDo1
⏳waiting until bootnode is up...
🏃starting node 12D3KooWRAsuiqzQKfjsCjTbwAnDkpDHzYdryz5DXhLEFPoN3akZ
🏃starting node 12D3KooWEjZfgGiVYAKNYAMcjg5f5mNygpJr2pwnfdjL4MVvT3xM
🏃starting node 12D3KooWJFKH86Gp2aTiWyPWnSDRdbwMRukbu8ct5nRp8LWDMd1V
🏃starting node 12D3KooWGDxLZK7LpG85TYMCJ5B5SqKGw39DrCHoS3DPFB1jsKp7
✔️cluster is running, bootnode is at /ip4/127.0.0.1/tcp/44763/p2p/12D3KooWLKKmDNUtuhD56qCmwecBmeN2q5QzhuQAuWekVffASDo1
```

See the command's help for the various parameters and what they do.

## State

The cluster node's states will by default be stored in a temporary directory that will be automatically destroyed when
the `nillion-devnet` process exits. The `--state-directory` parameter can be provided if a persistent directory is
desired.

## Identities

Node identities (e.g. private keys and peer ids) will be randomized by default. Use the `--identity-seed` parameter to
select a seed that will be used to generate all the node keys deterministically instead. Using this in combination with
`--state-directory` is useful if you want to test the same thing on the same cluster even if you re-run
`nillion-devnet`.
