# INV-RAN protocol

This protocol allow generating a tuple of shares $([a], [a^-1])$ such that the second element is the inverse of the
first one.

The way to do this is:
1. Run RAN to generate a share of a random element $[a]$ and $[b]$.
2. Ran MULT to multiply the elements together to product $[c] = [a] * [b]$.
3. Run REVEAL to reveal $[c]$ as $c$.
4. Abort if $c == 0$. This could happen if we're unlucky and RAN generated 0 for either $a$ or $b$.
5. Compute the inverse of $[a]$ as $[a^-1] = (c^-1 \ mod \ p) * [b]$.
6. Emit the tuple $([a], [a^-1])$.

The implementation in this crate allows generating _N_ tuples at once as that's the typical use case for INV-RAN.
