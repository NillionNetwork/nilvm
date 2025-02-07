# Nillion Client

Nillion client is a Rust library that exposes an interface to interact with Nillion network.
It uses the [client-core](../libs/client-core/README.md) crate for the core logic of the client and the node GRPC API
for interation with the node.

Clients for other languages should follow this pattern and use bindings to
the [client-core](../libs/client-core/README.md) crate plus GRPC code on
that language to interact with the nodes.
