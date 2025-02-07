# Integer Modulo with Secret Divisor protocol

This protocol state machine implements the protocol for Integer modulo with secret divisor and secret dividend. Take the following expression:

$$a = d\cdot \left\lfloor\frac{a}{d} \right\rfloor + (a\mod d).$$

## Protocol

1. Calculate dividend using $\texttt{DIVISION-SECRET-DIVISOR}$: $\left\lfloor\frac{a}{d}\right\rfloor$.
2. Multiply by the divisor using $\texttt{MULTIPLICATION}$: $d\cdot \left\lfloor\frac{a}{d}\right\rfloor$.
3. Subtract from the dividend: $a - d\cdot \left\lfloor\frac{a}{d}\right\rfloor$.
