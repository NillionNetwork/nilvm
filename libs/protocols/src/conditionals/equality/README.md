# Private Output Equality Protocol

This protocol enables the comparison between shares of two values $[x], [y] \in \mathbb{Z_p}$ outputing shares of $[1]$ if they are equal and shares of $[0]$ otherwise.

## Protocol Explanation

The protocol consists of two phases:

### 1. `PREP_PRIVATE_EQUALITY`

This state machine prepares the evaluation of the equality. The steps are the following:

1. For that it obtains a random number $r$ shared bitwise using $F_{RANDOM-BITWISE}$.
2. Produce a Lagrange polynomial of degree $p$.
3. Obtain the preprocessing to compute a the Lagrange Polynomial calling $F_{PREP-POLY-EVAL}$.

### 2. `PRIVATE_EQUALITY`

This state machine produces the private equality output for two numbers $x,y$ for which the users own shares $[x], [y]$. The steps are the following:

1. The parties compute the difference between the shares: $[z] = [x] - [y]$.
2. The parties mask the value $z$ by adding $r$: $[z_{masked}] = [z] + [r]$.
3. The parties reveal the shares of $[z_{masked}]$ to obtain the public value using $F_{REVEAL}$.
4. The parties compute the hamming distance between their shares of $[r]$ and the public value $z_{masked}$: $distance = HAMMING\_DISTANCE(r, z_{masked}) + 1$.
5. Finally, the Lagrange polynomial is evaluated on the resulting distance $F_{POLY-EVAL}(distance)$.

## Usage:

1. Call `prepPOLYEVAL(p)` to generate parameters for polynomial evaluation.
2. Use the returned values along with the polynomial coefficients to evaluate the polynomial at desired points using `POLY_EVAL(x, poly, prepInvPowers)`.

## Sub-protocols:

1. `RANDOM-BITWISE()`: Generates a random value `r` and produces bitwise shares of it.
2. `PREP-POLY-EVAL()`: Generates a set of preprocessing elements to evaluate a polynomial.
3. `TO-PUBLIC()`: Reveals the shares of a value to produce the reconstructed value.
4. `POLY-EVAL()`: Evaluates a polynomial at a specific point `x`.

## Pseudo-Code:

### Preprocessing:

```python
def prepEQUALS():
		r = random_bitwise()
		sequence = ((1,1), (2,0), ..., (p,0))
		poly = lagrange_polynomial(sequence)
		prepEvalPoly = prepEVALPOLY(len(poly))
		return r, poly, prepEvalPoly
```

### Online Phase:

```python
def xor(i, c, x):
    ci = bit(i, c)
    xi = bit(i, x)
    xor = ci + (1 - 2*ci) * xi
    return xor

def hammingDist(x, c):
    h = 0
    for i in range(L):
        h += xor(i, c, x)
    return h

def EQUALS(x, y, prepEquals):
		r, poly, prepEvalPoly = prepEQUALS()
    z = (x - y)
	  c = REVEAL(z + r.merge())
	  h = (hammingDist(r, c) + 1)
    powers = EVALPOLY(h, poly, prepEvalPoly)
    return e
```
