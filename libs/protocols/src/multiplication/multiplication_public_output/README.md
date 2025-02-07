# PUB-MULT protocol

This crate implements the share multiplication and reveal protocol state machine. This is referenced in the 
whitepaper as $F_\texttt{PUB-MULT}$.

The goal of this protocol is to reveal the inner product of two vectors of shares. It takes as input two vectors of size $n$, $([a_0], \dots, [a_{n-1}])$ and $([b_0], \dots, [b_{n-1}])$ and reveals the sum of the products $a_0 \cdot b_0 + \dots + a_{n-1} \cdot b_{n-1}$ without revealing any other information. Note that regular revealed multiplication is a special case of this when the given inputs only contain single elements $([a])$ and $([b])$, then the output is $a\cdot b$. This uses BGW style multiplication for passive adversaries, where:
1. Each node multiplies their shares $[a_i \cdot b_i] = [a_i] \cdot [b_i]$.
2. Each node sums their shares $[s] = \sum_i [a_i \cdot b_i]$.
3. Each node adds a pre-computed degree 2T zero share to result $[r] = [s] + [0]$.
4. Each node sends their result share $[r]$ to every other node.
5. Each node reconstructs the secret $r=\sum_{i=0}^{n-1}a_i \cdot b_i$ from the incoming shares.
