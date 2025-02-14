# threshold-keypair

Utilities for Threshold signing:
- Local keypair generation
- Serialization/De-serialization

## Serialization
This crate uses a non-self-describing format for serialization and deserialization. Non-self-describing formats, such as Bincode, do not include type metadata in the serialized output.

The `CoreKey` type, which is wrapped by `EcdsaPrivateKeyShare` in this crate, uses Serde's `deserialize_any` for deserialization. Since `deserialize_any` requires type information, it cannot be deserialized from non-self-describing formats like Bincode or other compact binary formats. Since Nillion's serialization method relies on Bincode, we need a tailored serialization for non-self describing format.