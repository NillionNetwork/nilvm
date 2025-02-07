# The Nillion CLI

This is a tool that uses the [Nillion Client](../../client-v2/README.md) to interact with the network, allowing you
to store/retrieve values, run computations, etc.

You can get a list of available commands and arguments executing:

```
$ nillion --help
```

You can specify default arguments to be used in a configuration file. Its default
path depends on the operating system:

* Linux: `$HOME/.config/nillion/config.yaml`
* macOS: `$HOME/Library/Application Support/com.nillion.nillion/config.yaml`
* Windows: `C:\Users\<User>\AppData\Roaming\nillion\nillion\config\config.yaml`

The values on that file are taken if not specified on the command line.
The contents of this file consist of a serialized `Cli` struct, for instance:

```
user_key_seed: "user_seed"
node_key_seed: "node_seed"
bootnodes:
- "/ip4/127.0.0.1/tcp/14111/p2p/12D3KooWCAGu6gqDrkDWWcFnjsT9Y8rUzUH8buWjdFcU3TfWRmuN"
- "/ip4/127.0.0.1/tcp/14112/p2p/12D3KooWMYauaGF4oZx1LSL9ntwKRfNpkTjwLmXjj6aqWbYYqBYh"
listen_address: "127.0.0.1"
command: InspectIds
```

### YAML example for `overwrite-permissions`

The `$ nillion [OPTIONS] overwrite-permissions` subcommand reads from a yaml file following the shape:

```yaml
retrieve:
  - <UserId>
update:
  - <UserId>
delete:
  - <UserId>
compute:
  <UserId>:
    - <ProgramId>
```

```yaml
# new-permissions.yaml
retrieve:
  - f35dae20722099910f95990ffe6f9399042579d7
update:
  - f35dae20722099910f95990ffe6f9399042579d7
delete:
  - f35dae20722099910f95990ffe6f9399042579d7
compute:
  f35dae20722099910f95990ffe6f9399042579d7:
    - 2LcqbSQfF5x1n6o8oxPaa36FU4cYcavALZA1St9sRXiLRNwQu3JL6ZyqwSShBTKyqXKpNf6Fxuwg5REFohSJgU5D/simple
```

To remove permissions provide an empty array or map. For example:

```yaml
# permissions-with-empty-values.yaml
retrieve:
  - f35dae20722099910f95990ffe6f9399042579d7
update:
  - f35dae20722099910f95990ffe6f9399042579d7
delete: [ ]
compute: { }
```

### YAML example for `update-permissions`

The `$ nillion [OPTIONS] update-permissions` subcommand reads from a yaml file following the shape:

```yaml
retrieve:
  grant:
    - <UserId>
  revoke:
    - <UserId>
update:
  grant:
    - <UserId>
  revoke:
    - <UserId>
delete:
  grant:
    - <UserId>
  revoke:
    - <UserId>
compute:
  grant:
    <UserId>:
      - <ProgramId>
  revoke:
    <UserId>:
      - <ProgramId>
```

```yaml
# new-permissions.yaml
retrieve:
  grant:
    - f35dae20722099910f95990ffe6f9399042579d7
delete:
  revoke:
    - f35dae20722099910f95990ffe6f9399042579d5
compute:
  grant:
    f35dae20722099910f95990ffe6f9399042579d7:
      - 2LcqbSQfF5x1n6o8oxPaa36FU4cYcavALZA1St9sRXiLRNwQu3JL6ZyqwSShBTKyqXKpNf6Fxuwg5REFohSJgU5D/simple
```
