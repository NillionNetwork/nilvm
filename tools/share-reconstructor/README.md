# Share reconstructor

This tool allows reconstructing a set of shares into the secret hidden behind them.

The shares/config is provided via a configuration file. See:
* The `config.prime_field.sample.yaml` sample file for the format used for prime field reconstructions.
* The `config.semi_field.sample.yaml` sample file for the format used for semi-field (2q) reconstructions.

Note that the prime number provided in the config file is $P$ regardless of whether the shares are in $P$ or $2q$.

## Usage

```shell
cargo run <config-file-path>
````
