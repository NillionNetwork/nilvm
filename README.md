# Nillion

Monorepo for the Nillion node, SDK, and client.

## Links

* [Node](./node/README.md)
* [Client](./client/README.md)
* SDK Tools
    * [nada](./tools/nada/README.md): A tool to manage Nada projects.
    * [nada-run](./tools/nada-run/README.md): A tool to run Nillion programs under a simulated network.
    * [nillion](./tools/nillion/README.md): A tool that uses the Nillion Client to interact with the network, allowing
      you to store/retrieve values, run computations, etc.
    * [nillion-devnet](./tools/nillion-devnet/README.md): A tool to run a local Nillion network.
    * [nilup](./tools/nilup/README.md): A tool to manage Nillion SDK versions.
    * [pynadac](./nada-lang/pynadac/README.md): The Nada language compiler.
* Main libs
    * [nada_dsl](./nada-lang/nada_dsl/README.md)
    * [nada-value](./libs/nada-value/README.md): A crate that models the data format behind Nada.
    * [math](./libs/math/README.md): Nillion mathematical library.
    * [protocols](./libs/protocols/README.md): Cryptographic protocols modeled as state machines.
    * [execution-engine](./libs/execution-engine/execution-engine-vm/README.md): A crate containing the logic for
      running computations in the Nillion network.
    * [client-core](./libs/client-core/README.md): Core logic for the Nillion Client, aimed to be reused in all clients
      of different languages.

## Setup

To set up the repository and tooling needed run

```bash
    $ ./setup_env.sh
```

and copy .env.sample to .env

```bash
cp .env.sample .env
```

modify `.env` to your preferences.