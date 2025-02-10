# IF-ELSE Protocol

This crate implements the oblivious selection between two secret values based on
a secret condition.

More specifically, given an encrypted condition result `cond`, and encrypted
values `a`, `b`, the `if_else` protocol can be invoked as:
```python
result = cond.if_else(a, b)
```

In this case, if `cond` is an encryption of `true` then `result` will be equal
to `a`, otherwise, `b`.
