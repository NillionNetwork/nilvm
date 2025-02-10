# Program simulator

This tool allows you to run a Nillion program under a simulated network.

## Usage

```bash
Usage: nada-run [OPTIONS] <PROGRAM_PATH>

Arguments:
  <PROGRAM_PATH>
          Program path

Options:
  -p, --prime-size <PRIME_SIZE>
          Prime size in bits
          
          [default: 256]

  -n, --network-size <NETWORK_SIZE>
          The size of the simulated network
          
          [default: 3]

  -d, --polynomial-degree <POLYNOMIAL_DEGREE>
          The degree of the polynomial used
          
          [default: 1]

  -i, --public-integer <INTEGERS>
          An integer public variable.
          
          These must follow the pattern `<name>=<value>`.

      --public-unsigned-integer <UNSIGNED_INTEGERS>
          An unsigned integer public variable.
          
          These must follow the pattern `<name>=<value>`.
          
          [aliases: ui]

      --secret-integer <SECRET_INTEGERS>
          An integer secret.
          
          These must follow the pattern `<name>=<value>`.
          
          [aliases: si]

      --secret-unsigned-integer <SECRET_UNSIGNED_INTEGERS>
          An unsigned integer secret.
          
          These must follow the pattern `<name>=<value>`.
          
          [aliases: sui]

      --array-public-integer <ARRAY_INTEGERS>
          An array of integer public variables
          
          The expected pattern is `<name>=<comma-separated-value>`.
          
          Example: array1=1,2,3
          
          [aliases: ai]

      --array-public-unsigned-integer <ARRAY_UNSIGNED_INTEGERS>
          An array of unsigned integer public variables
          
          The expected pattern is `<name>=<comma-separated-value>`.
          
          Example: array1=1,2,3
          
          [aliases: aui]

      --array-secret-integer <ARRAY_SECRET_INTEGERS>
          An array of integer secrets
          
          The expected pattern is `<name>=<comma-separated-value>`.
          
          Example: array1=1,2,3
          
          [aliases: asi]

      --array-secret-unsigned-integer <ARRAY_SECRET_UNSIGNED_INTEGERS>
          An array of unsigned integer secrets
          
          The expected pattern is `<name>=<comma-separated-value>`.
          
          Example: array1=1,2,3
          
          [aliases: asui]

      --secret-blob <SECRET_BLOBS>
          A blob secret.
          
          These must follow the pattern `<name>=<value>` and the value must be encoded in base64.
          
          [aliases: sb]

      --nada-values-path <NADA_VALUES_PATH>
          A path to load secrets from

  -h, --help
          Print help (see a summary with '-h')

  -V, --version
          Print version
```

