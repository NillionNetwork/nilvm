# Docker containers / compose for nillion network

Here are the containers that are artifacts from nillion network, like the node, functional test, load test, load test
report generator.

There is also the docker-compose file to "deploy" a network with five nodes, it also has other containers like load and
functional test inside to be run against the nillion network in docker compose.

:warning: IMPORTANT the containers used to build the software in this repository are in
[devops/docker](https://github.com/NillionNetwork/devops/tree/master/docker) repository and are used by
[jenkins](https://jenkins-internal.nilogy.xyz/job/nillion/)

## How to build and publish container

To build or publish the container we use `just` a tool like make that you can define recipes in the `justfile` and the
run them using `just`.

### How to build the containers

There is a recipe to build each container. To run the recipe execute in a shell in the root path of the repo

```bash
$ just docker-build-<container-name>
```

where `<container-name>` can be

* node
* load-test
* report-generator
* functional-tests

### How to publish the containers

:warning: In order to publish a container the aws credentials should be loaded in the environment.

There is a recipe to publish each container. To run the recipe execute in a shell in the root path of the repo

```bash
$ just docker-publish-<container-name>
```

where `<container-name>` can be

* node
* load-test
* report-generator
* functional-tests

## Docker Compose

### Running cluster

1. Build your node docker image:
    ```bash
    just docker-build-node
    ```
   or pull latest from AWS ECR:
   ```bash
   docker pull 592920173613.dkr.ecr.eu-west-1.amazonaws.com/nillion-node:latest
   ```

2. Run cluster:
    ```bash
    just docker-composer-up
    just docker-compose-down
    ```

### Running Functional Tests in cluster

1. Build your node with your changes:
    ```bash
    just docker-build-node
    ```
2. Execute functional tests:
    - Run all functional tests:
      ```bash
      just docker-run-functional-test
      ```
    - Or run a particular functional test:
      ```bash
      just docker-run-functional-test 'cargo test tests::test_retrieve_value::case_3_nil_transfer_integer'
      ```
    - Or get terminal access to Functional Test Container, so you can debug from there:
      ```bash
      just docker-run-functional-test bash
      apt update && apt install iputils-ping
      
      ping node-5
   
      cargo install libp2p-lookup
      libp2p-lookup direct --address /dns/node-5/tcp/14115
      
      RUST_LOG=debug cargo test tests::test_retrieve_value::case_3_nil_transfer_integer
      ```
3. Don't forget to terminate cluster when done with functional tests
    ```bash
    just docker-compose-down
    ```