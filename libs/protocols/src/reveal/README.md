# REVEAL protocol

This crate implements the REVEAL protocol state machine. This is referenced in the technical whitepaper
as $F_{REVEAL}$.

The goal of this protocol is to allow all nodes to transform their individual shares of a secret into a public
secret known by all nodes. This is achieved by letting each node broadcast its share of some element to every other
node and let each node reconstruct them once enough shares are gathered.

This is implemented generically over the field in which it operates. Therefore it serves both the implementation of
$F_{REVEAL_{2^k}}$ as well as the one for $F_{REVEAL_{p}}$, as per the whitepaper.
