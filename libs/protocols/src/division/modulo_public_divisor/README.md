# Modulo protocol

This crate implements both PREP-MODULO and MODULO protocol state machines.

## PREP-MODULO

The PREP-MODULO computes two elements: $k+\kappa$ shares of random bits and the preprocessing elements for the two comparisons.

## MODULO

The MODULO protocol is based on the deterministic protocol 3.5 from [Catrina and de Hoogh, 2010](https://citeseerx.ist.psu.edu/viewdoc/download?rep=rep1&type=pdf&doi=10.1.1.220.9499>), pag 8. Our implementation uses three protocols already integrated in the execution engine:  $F_{\texttt{RANDOM-BITWISE}}()$, $F_{\texttt{REVEAL}}()$ and $F_{\texttt{LESS-THAN}}()$.

The goal of this protocol is to compute in shared form a mod d, where d is public. It takes as inputs shared a and public d and outputs the share of a mod d. The protocol goes as follows:

1.  $m = \left \lceil{ \log d}\right \rceil$;
2.  $( [b_{0}''], \ldots, [b_{k+\kappa -m}''] )\leftarrow F_{\texttt{RANDOM-BITWISE}}(k+\kappa -m)$ and compute $[r''] = \sum_{i} [b_{i}'] \cdot 2^i$;
3.  $( [b_{0}'], \ldots, [b_{m}'] )\leftarrow F_{\texttt{RANDOM-BITWISE}}(m)$ and compute $[r'] = \sum_{i} [b_{i}'] \cdot 2^i$;
4.  $[b] = (2^{k-1} + [a] +  d \cdot [r''] + [r'])$;
5.  $c\leftarrow  F_{\texttt{REVEAL}}(\texttt{ALL}, [b])$;
6.  $c' = (c - 2^{k-1}) \mod d$;
7.  $[v] \leftarrow 1 - F_{\texttt{LESS-THAN}}([r'], d)$;
7.  $[u] \leftarrow F_{\texttt{LESS-THAN}}(c', [r'] - d\cdot [v])$;
9.  $[a'] = c' - [r'] + d\cdot ([v] + [u] )$;
10. Output $[a']$.
