# Node Onboarding

This document describes how to onboard a nilVM node onto one of the networks listed below.

> [!IMPORTANT]
> Onboarding for the below networks is invite-only and currently closed to new operators.

## Choose a Network

| Network Name    | Node/SDK Version | Node Configuration               |
| --------------- | ---------------- | -------------------------------- |
| nilvm-mainnet-1 | v0.9.0           | Coming soon...                   |
| nilvm-testnet-1 | v0.9.0           | [node.yaml][node-yaml-testnet-1] |
| nilvm-testnet-2 | v0.9.0           | [node.yaml][node-yaml-testnet-2] |

## Submit the Operator's Questionnaire

The questionnaire is intended to clarify requirements for operators and is intended to give Nillion
insight into the compatibility of a node operator's infrastructure. Read the
[questionnaire](./questionnaire.md) and submit your answers to your Nillion contact to begin
onboarding.

## Create an Identity

Download the network-compatible version of the [Nillion SDK][nillion-sdk] and use it to generate a
node identity; replace `NAME` with your or your company's name:

```bash
nillion identities add NAME
```

Use `show` to display the public key value:

```bash
nillion identities show NAME
```

_:bulb: The private key is located in `$XDG_CONFIG_HOME/nillion/identities/NAME.yaml` or
`$HOME/.config/nillion/identities/NAME.yaml`. You'll need it for instructions below._

## Add Node Configuration

Create a PR in this repo submitting your node's inputs under `cluster.members` in the relevant
network configuration file, e.g. [networks/nilvm_testnet_1.yaml](./networks/nilvm_testnet_1.yaml):

```yaml
cluster:
  members:
    - grpc_endpoint: https://domain.name.for.your.node
      public_keys:
        authentication: 023647adfcea675d584495f47af8d422feab582275d70fdb39c27577b64b2141fb
        kind: secp256k1
```

## Configure Your Node

Once your PR is approved and merged, use the network YAML file as the basis for your node's
config(`node.yaml`). You may customize a few of its fields with values specific to your deployment:

* `identity.private_key.path` - path to private key file (default: `/nillion/node.key`)

* `runtime.grpc.tls.cert` - path to certificate file (default: `/nillion/certbot-data/keys/letsencrypt/combined_cert.pem`)

* `runtime.grpc.tls.key` - path to certificate private key file (default: `/nillion/certbot-data/keys/letsencrypt/privkey.pem`)

* `storage.object_storage.aws_s3.bucket_name` - Bucket name for object storage (default: `nilvm-testnet-1-storage`; network dependent)

Replace `payments.rpc_endpoint` with a dedicated, highly-available network endpoint provided by your
Nillion contact.

> [!NOTE]
> A nilVM node listens on 2 network ports: `14311` (gRPC server) and `34111` (Prometheus metrics)
> and need to be exposed appropriately.

### MinIO Configuration

Node operators using [MinIO](https://min.io/) as the backing object store may specify additional
configuration under  `storage.object_storage.aws_s3`, e.g.:

```
object_storage:
    aws_s3:
        allow_http: true
        endpoint_url: http://localhost:9000
        region: us-east-1
```

In addition, the standard `AWS_ACCESS_KEY_ID` and `AWS_SECRET_ACCESS_KEY` environment variables can
be supplied to the node container to ensure authentication against MinIO is possible.

## Launch Your Node

Pull the network-compatible version of the node Docker image from Nillion's AWS ECR
[repository](public.ecr.aws/k5d9x2g2/nilvm); remember to append `-amd64` to the version number
tag when pulling, e.g.:

> [!IMPORTANT]
> Substitute `${VERSION}`, below, with the Node/SDK version for the corresponding network found in
> the networks list above.

```bash
docker pull public.ecr.aws/k5d9x2g2/nilvm:${VERSION}-amd64
```

Then, launch the node on your infrastructure using the certificates, Docker image, node
configuration and private key mentioned above. The node binary packaged into the Docker image
expects a `CONFIG_PATH` environment variable to be set to the path of node configuration, e.g.:

_This command is not meant to be the exact Docker `run`  command to run on your infra. It's only
illustrative for the environment variable and ECR URL._

```bash
docker run -e CONFIG_PATH=/etc/nillion/node.yaml public.ecr.aws/k5d9x2g2/nilvm:${VERSION}-amd64
```

All nodes present in `cluster.members` must be available before network functions can be validated.

[nillion-sdk]: https://docs.nillion.com/nillion-sdk-and-tools
[node-yaml-mainnet-1]: ./networks/nilvm-mainnet-1.yaml
[node-yaml-testnet-1]: ./networks/nilvm-testnet-1.yaml
[node-yaml-testnet-2]: ./networks/nilvm-testnet-2.yaml
