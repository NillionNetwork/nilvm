# hamming_distance.py
# An example program that calculates the Hamming distance between two arrays of secret integers
# Hamming distance = Sum( v_i != u_i )
from nada_dsl import Array, Input, Integer, Output, Party, SecretInteger, Tuple


def nada_main():
    party1 = Party(name="Party1")
    v = Array(SecretInteger(Input(name="my_array_1", party=party1)), size=3)
    u = Array(SecretInteger(Input(name="my_array_2", party=party1)), size=3)
    secret_int0 = SecretInteger(Input(name="secret_int0", party=party1))

    def is_equal(values: Tuple[SecretInteger, SecretInteger]) -> SecretInteger:
        return (values.left == values.right).if_else(Integer(0), Integer(1))

    def add(x: SecretInteger, y: SecretInteger) -> SecretInteger:
        return x + y

    # Computes A != B element-wise
    is_eq = u.zip(v).map(is_equal)

    # Adds all terms of is_eq to compute the hamming distance, i.e., the number
    # of different elements.
    distance = is_eq.reduce(add, secret_int0)

    return [Output(distance, "out", party1)]
