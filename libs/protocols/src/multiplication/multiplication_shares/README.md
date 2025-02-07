# MULT protocol

This crate implements the share multiplication protocol state machine. This is referenced in the whitepaper as
$F_\texttt{MULT}$.

The goal of this protocol is to take the inner product of two vectors of shares. It takes as input two vectors of size $n$, $([a_0], \dots, [a_{n-1}])$ and $([b_0], \dots, [b_{n-1}])$ and outputs the share of the sum of the products $[a_0 \cdot b_0 + \dots + a_{n-1} \cdot b_{n-1}]$. Note that regular multiplication is a special case of this when the given inputs only contain single elements $([a])$ and $([b])$ and then the output is $[a\cdot b]$. This uses GRR style multiplication for passive adversaries, where:
1. Each node multiplies their shares $[a_i \cdot b_i] = [a_i] \cdot [b_i]$.
2. Each node sums their shares $[s] = \sum [a_i \cdot b_i]$.
3. Each node hides the resulting share in a polynomial of degree T and hands out a share to every other node.
4. Each node locally computes their share of the product by interpolating incoming shares.
