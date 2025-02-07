from nada_dsl import *


def nada_main():
    party1 = Party(name="Party1")
    a = SecretUnsignedInteger(Input(name="A", party=party1))
    b = PublicUnsignedInteger(Input(name="B", party=party1))
    c = PublicUnsignedInteger(Input(name="C", party=party1))

    result = a + (b ** c)

    return [Output(result, "my_output", party1)]
