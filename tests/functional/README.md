# Functional test

Functional test to run against the Nillion Network.

## Nodes Configuration

The test by default spawns the Nillion nodes as different processes in the same machine, but this behaviour can be 
changed using environment variables.

### Don't spawn nodes and run against remote or already running nodes

Use the environment variable `REMOTE_NODES=/path/to/remote-nodes.config.yml` pointing to a configuration file that tells 
the test where the nodes are. Look at `tests/resources/config/...` to see examples of this file.


