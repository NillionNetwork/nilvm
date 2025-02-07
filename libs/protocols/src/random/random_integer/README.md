# Random Integer protocol

This crate implements the RAN protocol state machine. This is referenced in the whitepaper as
$F_{P-RAND}$.

This protocol allows all nodes to collectively generate a random unknown number among them. The way to do this is:

1. Each node generates a random number locally.
2. The node hides it in a degree $T$ polynomial and hands out shares of it to all nodes.
3. All nodes locally combine they own share with everyone else's by multiplying with a hyper-invertible matrix.
4. The result is a vector of shares of unknown random numbers.