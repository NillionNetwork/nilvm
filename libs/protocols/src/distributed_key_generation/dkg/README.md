# Distributed Key Generation (DKG) Protocol

- This protocol implements the Distributed Key Generation (DKG) protocol from the [cggmp21](https://docs.rs/cggmp21/0.5.0/cggmp21/) library.
- It generates shares of an ECDSA private key that are distributed among multiple parties, enabling threshold signing capabilities.
- Each party receives their own private key share while maintaining the security of the overall scheme.

## Notes on the implementation

The protocol follows a slightly different logic compared to other MPC protocols in the `protocols` crate due to certain limitations of the state machine API provided by the cggmp21 library. In summary, we use Nillion's state machine to wrap the cggmp21 state machine, which is executed in a separate thread.
