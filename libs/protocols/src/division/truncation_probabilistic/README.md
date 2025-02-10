# Truncation Probabilistic protocol

This crate implements TRUNCPR protocol state machines. This protocol performs truncation using probabilistic methods.


## TRUNCATION-PROBABILISTIC

The TRUNCATION-PROBABILISTIC protocol is based on protocol 3.1 from [Catrina and de Hoogh, 2010](https://www.researchgate.net/profile/Octavian-Catrina/publication/225092133_Improved_Primitives_for_Secure_Multiparty_Integer_Computation/links/5aa17be7aca272d448b3724b/), page 6. Our implementation uses two protocols already integrated in the execution engine: $F_{\texttt{RANDOM-BITWISE}}()$, $F_{\texttt{REVEAL}}()$.

The goal of this protocol is to compute in shared form $\lfloor a/2^{m}\rfloor$, where $m$ is public and $a$ is shared. The protocol goes as follows:

1.  $([b_{0}], \ldots, [b_{k+\kappa-1}] )\leftarrow F_{\texttt{RANDOM-BITWISE}}(k+\kappa)$
- $[r] = \sum_{i=0}^{k+\kappa-1} [b_{i}] \cdot 2^i$;
- $[r'] = \sum_{i=0}^{m-1} [b_{i}] \cdot 2^i$;
2.  $[b] = (2^{k-1} + [a] + [r])$;
3.  $c\leftarrow  F_{\texttt{REVEAL}}(\texttt{ALL}, [b])$;
4.  $c' = c \mod 2^m$;
5.  $[a'] = c' - [r']$;
6. $[d] = ([a] - [a'])(2^{-m} \mod q)$;
7. Output $[d]$.

