# Polynomial Evaluation Protocol

This protocol enables the secure evaluation of a polynomial.

You can find more details on the [feature page](https://www.notion.so/nillion/Equality-d528821a31804314880f5050f6511416?pvs=4).

## Protocol Explanation

The protocol consists of two phases:

### 1. `PREP_POLY_EVAL`

This function prepares the necessary parameters for evaluating a polynomial of degree `p`. It generates a random value `r` and its inverse `r_inv`, computes powers of `r` up to `p`, and generates a random sharing `zero`.

### 2. `POLY_EVAL`

This function evaluates the polynomial at a given point `x`. It first performs a public multiplication of `x` with `r_inv` and `zero`. Then, it locally computes the polynomial value using the provided coefficients `poly` and precomputed powers of `r`.

## Usage:

1. Call `prepPOLYEVAL(p)` to generate parameters for polynomial evaluation.
2. Use the returned values along with the polynomial coefficients to evaluate the polynomial at desired points using `POLY_EVAL(x, poly, prepInvPowers)`.

## Sub-protocols:
1. `INV_RAN()`: Generates a random value `r` and its inverse `r_inv`.
2. `RAN_ZERO()`: Generates a random sharing of 0 (`zero`) used for secure computation.
3. `MULT(a, b)`: Performs secure multiplication of two values `a` and `b`.
4. `PUB_MULT(x, r_inv, zero)`: Performs public multiplication of a value `x` with a precomputed inverse `r_inv` and uses a random value `zero` in $2T$.


## Pseudo-Code:

### Preprocessing:

```python
def prepPOLYEVAL(p):
    r, r_inv = INV_RAN()
    r_powers = [1, r]
    s = r
    for i in range(p-2):
        s = MULT(s, r)
        r_powers.append(s)
    zero = RAN_ZERO()
    return r_inv, r_powers, zero

```
### Online Phase:

```python
def POLYEVAL(x, poly, prepInvPowers):
    c = PUB_MULT(x, prepInvPowers.r_inv, prepInvPowers.zero)
    # LOCAL
    pow_c = 1
    poly_x = 0
    for poly_i, r_power in zip(poly, prepInvPowers.r_powers):
        xs = (pow_c * r_power)
        poly_x += poly_i * xs
        pow_c *= c
    return poly_x

```