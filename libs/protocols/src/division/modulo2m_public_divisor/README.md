# Modulo2m protocol

This crate implements both PREP-MODULO2M and MODULO2M as well as TRUNCATION protocol state machines.
MODULO2M is a building block for many important operations such as truncation (integer division by a power of two) or square-root.

The state machine can be switched from MODULO2M to TRUNCATION protocol via a configuration parameter.

## PREP-MODULO2M

The PREP-MODULO2M computes two elements: $k+\kappa$ shares of random bits and the preprocessing elements for one comparison.

## MODULO2M

The MODULO2M protocol is based on the deterministic protocol 3.2 from [Catrina and de Hoogh, 2010](https://citeseerx.ist.psu.edu/viewdoc/download?rep=rep1&type=pdf&doi=10.1.1.220.9499>), page 7. Our implementation uses three protocols already integrated in the execution engine: $F_{\texttt{RANDOM-BITWISE}}()$, $F_{\texttt{REVEAL}}()$, and $F_{\texttt{LESS-THAN}}()$.

The goal of this protocol is to compute in shared form $a \mod 2^m$, where $m$ is public and $a$ is shared. The protocol goes as follows:

1.  $([b_{0}''], \ldots, [b_{k+\kappa -m}''] )\leftarrow F_{\texttt{RANDOM-BITWISE}}(k+\kappa -m)$ and compute $[r''] = \sum_{i} [b_{i}''] \cdot 2^i$;
2.  $( [b_{0}'], \ldots, [b_{m}'] )\leftarrow F_{\texttt{RANDOM-BITWISE}}(m)$ and compute $[r'] = \sum_{i} [b_{i}'] \cdot 2^i$;
3.  $[b] = (2^{k-1} + [a] +  2^m \cdot [r''] + [r'])$;
4.  $c\leftarrow  F_{\texttt{REVEAL}}(\texttt{ALL}, [b])$;
5.  $c' = c \mod d$;
7.  $[u] \leftarrow F_{\texttt{LESS-THAN}}(c', [r'])$;
9.  $[a'] = c' - [r'] + 2^m\cdot [u]$;
10. Output $[a']$.

## TRUNCATION

The TRUNCATION protocol is based on protocol 3.3 from [Catrina and de Hoogh, 2010](https://citeseerx.ist.psu.edu/viewdoc/download?rep=rep1&type=pdf&doi=10.1.1.220.9499>), page 7. It uses the output of MODULO2M. Our implementation uses three protocols already integrated in the execution engine: $F_{\texttt{RANDOM-BITWISE}}()$, $F_{\texttt{REVEAL}}()$, and $F_{\texttt{LESS-THAN}}()$.

The goal of this protocol is to compute in shared form $\lfloor a/2^{m}\rfloor$, where $m$ is public and $a$ is shared. The protocol goes as follows:

1.  $([b_{0}''], \ldots, [b_{k+\kappa -m}''] )\leftarrow F_{\texttt{RANDOM-BITWISE}}(k+\kappa -m)$ and compute $[r''] = \sum_{i} [b_{i}''] \cdot 2^i$;
2.  $( [b_{0}'], \ldots, [b_{m}'] )\leftarrow F_{\texttt{RANDOM-BITWISE}}(m)$ and compute $[r'] = \sum_{i} [b_{i}'] \cdot 2^i$;
3.  $[b] = (2^{k-1} + [a] +  2^m \cdot [r''] + [r'])$;
4.  $c\leftarrow  F_{\texttt{REVEAL}}(\texttt{ALL}, [b])$;
5.  $c' = c \mod d$;
7.  $[u] \leftarrow F_{\texttt{LESS-THAN}}(c', [r'])$;
9.  $[a'] = c' - [r'] + 2^m\cdot [u]$;
10. $[d] = ([a] - [a'])(2^{-m} \mod q)$;
11. Output $[d]$.