# Unbounded multiplication protocols

This implements all of the protocols that require unbounded multiplication. That is, $F_{MULT}$ over an arbitrary
number of inputs, $F_{PREFIX-MULT}$, and $F_{BINOM-MULT}$.

All of these have a "preparation" piece in common that gets some tuples of invertible numbers from INV-RAN and
uses the simpler $F_{MULT}$ over two operands to multiply them together twice, plus a REVEAL operation.
