# RAN-BIT protocol

This crate implements the RAN-BIT protocol state machine. This is referenced in the technical whitepapers
as $F_{RAN-BIT}$.

This protocol generates shares for N random values, where the numbers can only be zeroes or ones.

# RANDOM-BITWISE protocol

This crate implements the RANDOM-BITWISE protocol state machine. This is referenced in the technical whitepapers
as $F_{RANDOM-BITWISE}$.

This protocol generates N numbers where each of them is represented as a vector of shares of its bits. That is,
for N=1 and a 128 bit prime, this produces 128 shares.

## RAN-QUATWISE protocol

This protocol generates a random bitwise shared number as well as products of i-th and i+1-th term.

This is used for pre-processing quaternary operations.