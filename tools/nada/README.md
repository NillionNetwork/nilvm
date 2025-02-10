# Nada

## Description

Tool to manage nada projects.
It can:

- Create a new project
- Compile nada programs
- Run nada programs (also in debug mode)
- Test a program (also in debug mode)
- Generate a test from a program
- Output the compute requirements for a program
- Benchmark the execution of several programs
- Publish programs to a live Nillion network
- Execute programs in a live Nillion network

## Requirements

Python and `nada_dsl` are required to compile nada programs. Is recommended to use a python virtual environment to
install `nada_dsl`.

## Usage

```bash
Usage: nada <COMMAND>

Commands:
  init                  Create a new nada project
  build                 Build a program
  run                   Run a program using the inputs from a test case
  test                  Run tests
  benchmark             Benchmark one or multiple programs
  generate-test         Generate a test for a program with example values
  program-requirements  Get requirements for program
  shell-completions     Generate shell completions
  list-protocols        List Protocols (instructions) in JSON format
  publish               Publish a nada program
  compute               Compute a nada program in a live network
  help                  Print this message or the help of the given subcommand(s)

Options:
  -h, --help     Print help
  -V, --version  Print version

```

### Example flow

```bash
$  nada init my-nada-project
Creating new nada project: my-nada-project
Project created!

$ cd my-nada-project

$ ls -R
.:
nada-project.toml  src  target  tests

./src:
main.py

./target:

./tests:

$ cat nada-project.toml
name = "my-nada-project"
version = "0.1.0"
authors = [""]

[[programs]]
path = "src/main.py"
prime_size = 128

$ cat src/main.py
from nada_dsl import *

def nada_main():
    party1 = Party(name="Party1")
    party2 = Party(name="Party2")
    party3 = Party(name="Party3")
    a = SecretInteger(Input(name="A", party=party1))
    b = SecretInteger(Input(name="B", party=party2))

    result = a + b

    return [Output(result, "my_output", party3)]

$ nada build
Building program: main
Build complete!

$ nada generate-test --test-name my-main-test main
Generating test 'my-main-test' for
Building ...
Generating test file ...
Test generated!

$ cat tests/my-main-test.yaml
---
program: main
inputs:
  secrets:
    A:
      SecretInteger: "3"
    B:
      SecretInteger: "3"
expected_outputs:
  my_output:
    SecretInteger: "3"

$ nada run my-main-test
Running program 'main' with inputs from test file my-main-test
Building ...
Running ...
Program ran!
Outputs: {
    "my_output": SecretInteger(
        NadaInt(
            6,
        ),
    ),
}

$ nada test my-main-test
Running test: my-main-test
Building ...
Running ...
my-main-test: FAIL
Output 'my_output' expected SecretInteger(NadaInt(3)) but got SecretInteger(NadaInt(6))

$ nada run my-main-test -d
Running program 'main' with inputs from test file my-main-test
Building ...
Running ...
[Heap 1] main.py:7 a = SecretInteger(Input(name="A", party=party1)) -> load [Input 0] <= SecretInteger(3 mod 340282366920938463463374607429104828419)
[Heap 2] main.py:8 b = SecretInteger(Input(name="B", party=party2)) -> load [Input 1] <= SecretInteger(3 mod 340282366920938463463374607429104828419)
[Operation 3] main.py:10 result = a + b => SecretInteger(3 mod 340282366920938463463374607429104828419) + SecretInteger(3 mod 340282366920938463463374607429104828419)  = SecretInteger(6 mod 340282366920938463463374607429104828419)
Program ran!
Outputs: {
    "my_output": SecretInteger(
        NadaInt(
            6,
        ),
    ),
}
```

## Test Frameworks

### Use a Test Framework

To use a test framework, add the following to the `nada-project.toml` file:

```toml
[[test_framework.my-test-framework-name]]
command = "my-test-framework-command ./tests"
```

We recommend to use nada-test python package as a test framework. To use it, add the following to
the `nada-project.toml` file:

pip install nada-test

```toml
[[test_framework.nada-test]]
command = "nada-test ./tests"
```

### Create your own Test Framework

To create your own test framework you need a command that `nada test` will call base on the configuration in
the `nada-project.toml` file.
It will provide the environment variables

- `NADA_PROJECT_ROOT` it is a path to the root of the project
- `NADA_TEST_COMMAND` it the command that should be executed, will explain later each command
- `NADA_TEST_NAME` it is the name of the test that the command applies to

#### Commands

##### `NADA_TEST_COMMAND=list`

The test framework should list all the tests that are available in the project. The output should be a json array of
objects
with the next structure:

```json
[
  {
    "name": "<test name>",
    "program": "<program name>"
  }
]
```

##### `NADA_TEST_COMMAND=inputs`

The test framework should output the inputs of the test in json format via stdout for the test defined in the env
var `NADA_TEST_NAME`

##### `NADA_TEST_COMMAND=test`

The test framework should run the test in the env var `NADA_TEST_NAME` adn exit with a susceessful exit code i it passed
or a non-zero exit code if it failed.
stdout and stderr will be captured by nada and displayed to the user if the test fails.

# Network configurations

It is possible to interact with live networks using the `nada` tool. For this, you can configure networks in your
`nada-project.toml`. This is done adding sections like this one:

```toml
[networks.devnet]
identity = "my_identity"
```

The name of the network matches the name of a network configuration file. In our example, `nada` would look for a file
called `devnet.yaml` in the network configuration folder (`$HOME/.config/nillion/network`). The file contains the
parameters that the Nillion client requires to access the network.

This is an example:

```yaml
bootnode: http://127.0.0.1:61759
payments:
  nilchain_rpc_endpoint: http://localhost:26648
  nilchain_private_key: 9a975f567428d054f2bf3092812e6c42f901ce07d9711bc77ee2cd81101f42c5
  gas_price: null
```

Currently, `nillion-devnet` will generate this file automatically for you for every run.

In order to access the network a set of identities are required as well, your user private key and the node private key.
In the example above, `nada` would look for the file `my_identity.yaml` under the identities configuration folder (
`$HOME)/.config/nillion/identities`). The file looks like this:

```yaml
user_key: XXXXXXXXXX
```

You can use `nillion identity-gen` to generate an identity file automatically.

# Computing test programs

The `nada compute` command allows you to run a computation in a live network. Assuming you have a corresponding network
configuration and a test, `nada` will collect the values of the program inputs from the corresponding test configuration
and run a computation
in the selected network.

This is a complex command that performs the following actions:

- Builds the program
- Publishes the program in the provided network
- Runs the computation in the provided network.

## Usage

```
Compute a nada program in a live network

Usage: nada compute --network <NETWORK> <TEST>

Arguments:
  <TEST>  The test name to be run

Options:
  -n, --network <NETWORK>  A valid network name in the configuration
  -h, --help               Print help
```

## Example

Let's say you have a test called `mytest`, and a network configuration called `mynetwork`, you can execute the test
doing: `nada compute -n mynetwork mytest`. The output looks something like this:

```
Building ...
Running program 'mytest' on network: mynetwork, with inputs from test file mytest
Build complete!
Publishing: mytest
Payments transaction hash: C408ADF15D2C01B71208E99561F5D4F01442910B264B98D180EBF80A0161E7CB
Computing: 3B4fFmZDKkYdTwdUXZA8KUKTRbr163DHJ4AHSr2EZFGyNLKU5q9Nd5CzejHTtvRaJHrPFtMpKTHL7f67DYryKuzw/mytest-ZAXI5mz3
Payments transaction hash: 49B3FE109F44B479A057897CD886A34D7790B4EA0670F32FDF8F6733FBD694BD
Output (out1): SecretInteger(NadaInt(0))
Output (out2): SecretInteger(NadaInt(3))
Output (out3): SecretInteger(NadaInt(3))
```
