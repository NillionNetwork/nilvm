# BIT-DECOMPOSITION Protocol

This crate implements the bit decomposition protocol.

## Bit Less Than

Bit Less Than protocol compares a public value to a bitwise secret shared value
by recursively combining less and equals operations [1].

```
def bitLessThan(public, secret):
    length = len(secret) // 2
    # Setup
    ands = [bit(2*i, secret) * bit(2*i+1, secret) for i in range(length)]
    # Bitwise Comparison
    less = [lessThan(i, public, secret, ands) for i in range(length)]
    equal = [equals(i, public, secret, ands) for i in range(length)]
    # Recursive Reduction
    while length > 1:
        length = length // 2
        less = [less[2*i+1] + less[2*i] * equal[2*i+1] for i in range(length)]
        equal = [equal[2*i] * equal[2*i+1] for i in range(length)]
    # Result
    return less[0]
```

## Bit Adder

Carry Look Ahead Bit Adder is a protocol for adding L=T::MODULO.bits() carry
adder by using a carry look ahead, then using these carries to add up all the
values [2]. This protocol is the building block for the next two protocols.

```
    A0 B0    A1 B1           An Bn
     | |      | |             | |
C0  - + - C1 - + - C2 ... Cn - + - S{n+1}
      |        |               |
      S0       S1              Sn

Si <- (Ai ^ Bi) ^ Ci

C{i+1} <- Ai & Bi + (Ai ^ Bi) & Ci
```

```
def bitAdd(left, right):
    carry = [bit(i, left) * bit(i, right) for i in range(L)]
    propagate = [bit(i, left) + bit(i, right) - 2 * carry[i] for i in range(L)]
    for i in range(logL):
        carry = [carry[j] + ((j // (2**i)) % 2) * mult(carry[2**i * (j // 2**i) - 1], propagate[j]) for j in range(L)]
        propagate = [mult(propagate[j], propagate[2**i * (j // 2**i) - 1]**((j // 2**i) % 2)) for j in range(L)]
    return [bit(0, left) + bit(0, right) - 2 * bit(0, carry)] + \
        [bit(i, left) + bit(i, right) + bit(i-1, carry) - 2 * bit(i, carry) for i in range(1, L)]
```

## Mixed Bit Adder

Mixed Bit Adder is a protocol for adding a clear value to a bitwise shared
secret value. The products Ai & Bi = Ai * Bi can be calculated locally.

## Secret Bit Adder

Secret Bit Adder is a protocol for adding two secret bitwise shared values
together. The products Ai & Bi = Ai * Bi have to be calculated via invoking the
mult protocol.

## Bit Decompose

Given a secret shared number, Bit Decompose returns L shares each representing a
bit of the original number [2, 3].

```
def bitDecompose(input, solvedBits):
    revealed = (input - solvedBits) % PRIME
    less = bitLessThan(PRIME - revealed - 1, solvedBits)
    diff = [(bit(i, 2**L + revealed - PRIME) - bit(i, revealed)) * less + bit(i, revealed) for i in range(L)]
    bits = bitAdd(solvedBits, diff)
    return bits
```

## References

[1] Malten, W., Ugurbil, M., & de Vega, M. (2023). More efficient comparison
protocols for MPC. Cryptology ePrint Archive.

[2] Damgård, I., Fitzi, M., Kiltz, E., Nielsen, J. B., & Toft, T. (2006, March).
Unconditionally secure constant-rounds multi-party computation for equality,
comparison, bits and exponentiation. In Theory of Cryptography Conference (pp.
285-304). Berlin, Heidelberg: Springer Berlin Heidelberg.

[3] Nishide, T., & Ohta, K. (2007). Multiparty computation for interval,
equality, and comparison without bit-decomposition protocol. In Public Key
Cryptography–PKC 2007: 10th International Conference on Practice and Theory in
Public-Key Cryptography Beijing, China, April 16-20, 2007. Proceedings 10 (pp.
343-360). Springer Berlin Heidelberg.
