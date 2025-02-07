# Nillion Node

## Configuration

The preferred way of configuring the node is by using a yaml configuration file. See the `node.config.yaml.sample`
file as an example. Besides the sample configuration file, the generated documentation contains descriptions of
every possible property.

### Environment variables

Environment variables can be used as a way of configuring the node, and can also be used in conjunction
with a configuration file to override certain properties.

In order to find out what's the environment variable that corresponds to a particular key in the configuration file,
simply take its path in the yaml file, uppercase it, and separate it with hyphens. For example:

```
identity:
  node_id: b8fda536-486b-49b1-b606-bd4fef190669
```

To override the above parameter, the environment variable name should be `IDENTITY__NODE_ID`. That is, names should
be set as-is, and nesting is expressed via double underscores.

## sqlite

### Migration scripts

All migration scripts use the [sqlx-cli](https://github.com/launchbadge/sqlx/blob/main/sqlx-cli/README.md) and are
applied automatically when the node starts.




