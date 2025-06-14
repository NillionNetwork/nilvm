# Load test

This tool allows running tests against the Nillion network that perform the same operation (e.g. store values) over and
over again in order to check the performance of the network.

## Usage

This takes a file that defines what test to run. See the example yaml files in this directory as a guide.

```bash
This tool allows creating load on the network in a user defined way.

Usage: load-tool [OPTIONS] --spec-path <SPEC_PATH> --bootnode <BOOTNODE> --cluster-id <CLUSTER_ID> 

Options:
  -s, --spec-path <SPEC_PATH>
          Load test file

  -o, --output-path <OUTPUT_PATH>
          Output file path

  -b, --bootnode <BOOTNODE>
          The bootnode to use

  -c, --cluster-id <CLUSTER_ID>
          The cluster id to use

  -v, --verbose
          Enable verbose client output

  -h, --help
          Print help (see a summary with '-h')

  -V, --version
          Print version

```

## Testing concurrent users

The load tool can be used to test the network with a number of concurrent users. However, this comes with a few caveats:

- Manual and Automatic modes do not support progressively increasing number of clients. Clients number is fixed and will
  not change during the test.

**Manual:**
In this node number of clients is equal to a initial_workers parameter of the spec file.
Bear in mind that the number of clients is fixed and will not change during the test. Meaning clients will be re-used
across workers.

**Automatic mode:**
In this mode only one client is used, so it will be re-used across workers.

**Steady mode:**
In this mode number of Nillion Clients by default is equal to number of workers (you can override clients number by
setting `clients` parameter of this mode in the spec file).
