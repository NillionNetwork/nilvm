# Client Core

Core logic for the Nillion Client, aimed to be reused in all clients of different languages.

If you are looking for the Nillion Client, check the [Client](../../client-v2/README.md) crate.

If you want to create a new client for a new language, you can use this crate as a base.
At high level the steps to create a new client are:

* Create bindings to the client core in the new language.
* Use the GRPC of the node to communicate the new client with the node.
* Use the bindings to the client core as he main logic of the client.
* Glue together GRPC and the client core.

You can see examples of this in the [python](../../client/bindings/python/README.md)
and [typescript](https://github.com/NillionNetwork/client-ts) clients. 