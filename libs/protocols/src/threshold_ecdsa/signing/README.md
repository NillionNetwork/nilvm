# Signing protocol

- This protocol is the main singing protocol of the Threshold ECDSA Signing protocol from the [cggmp21](https://docs.rs/cggmp21/0.5.0/cggmp21/) library. 
- It generates shares of a signature that then have to be reconstructed by the client.

## Notes on the implementation

The protocol follows a slightly different logic compared to other MPC protocols in the `protocols` crate due to certain limitations of the state machine API provided by the cggmp21 library. In summary, we use Nillion's state machine to wrap the cggmp21 state machine, which is executed in a separate thread.
