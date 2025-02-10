# Integer Division with Secret Divisor protocol

This protocol state machine implements the protocol for Integer division with Secret divisor (DIVISION-SECRET-DIVISOR). And its corresponding Pre-processing protocol (PREP-DIVISION-SECRET-DIVISOR).

## Protocol

This is a Python prototype of the actual protocol

```python

def secret_integer_division(g, a, f):
    # State 0: Sign Comparison
    s_a = a < 0
    s_g = g < 0

    # State 1: Sign Multiplication
    a = a * (1 - 2*s_a)
    g = g * (1 - 2*s_g)
    s = 1 - s_a - s_g + 2 * s_a * s_g

    # State 2: Scale
    v = scale(a, f)

    # State 3: Scale Multiplication
    b = a * v
    w = g * v

    # Initial Guess
    alpha = 3/2 - sqrt(2)
    t = ceil(log2(-f / log2(alpha)))
    c = round((3 - alpha) * 2**f) - 2 * b

    # Newton-Raphson
    # State 4: Recursive Multiply & Truncate twice
    for i in range(t):
        z = 2**(f+1) - truncPR(c * b, f)
        c = truncPR(c * z, f)

    # State 5: Multiply & Truncate
    q = truncPR(c * w, f)

    # State 6: Deterministic Truncate
    q = trunc(q, f)

    # State 7: Estimate Multiplication
    e = q * a
    o = s * (a - 1)

    # State 8: Estimate Comparison
    l = g < e
    h = (e + o) < g

    # Correction
    q = q - l + h

    # State 9: Sign Multiplication
    q = q * (2*s - 1)

    return q
```
