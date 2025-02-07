# Auxiliary information protocol

- This protocol is part of the Threshold ECDSA Signing protocol from the [cggmp21](https://docs.rs/cggmp21/0.5.0/cggmp21/) library. 
- The protocol only needs to be generated once during cluster creation and can be reused for each signing request.
- The protocol generates a Paillier encryption key pair for each party while ensuring that specific properties of the private key are satisfied.

## Notes on the implementation

The protocol follows a slightly different logic compared to other MPC protocols in the `protocols` crate due to certain limitations of the state machine API provided by the cggmp21 library. In summary, we use Nillion's state machine to wrap the cggmp21 state machine, which is executed in a separate thread.
