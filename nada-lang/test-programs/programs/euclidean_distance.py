from nada_dsl import *


def nada_main():
    party1 = Party(name="Party1")
    A = Array(SecretInteger(Input(name="my_array_1", party=party1)), size=3)
    B = Array(SecretInteger(Input(name="my_array_2", party=party1)), size=3)
    secret_int0 = SecretInteger(Input(name="secret_int0", party=party1))

    def sub(values: Tuple[SecretInteger, SecretInteger]) -> SecretInteger:
        return values.left - values.right

    def add(x: SecretInteger, y: SecretInteger) -> SecretInteger:
        return x + y

    def mul(values: Tuple[SecretInteger, SecretInteger]) -> SecretInteger:
        return values.left * values.right

    # Computes A - B element-wise
    diff = A.zip(B).map(sub)
    # Computes (A - B) * (A - B) element-wise
    square_diff = diff.zip(diff).map(mul)
    # Adds all terms of (A - B) * (A - B)
    distance = square_diff.reduce(add, secret_int0)

    return [Output(distance, "out", party1)]
