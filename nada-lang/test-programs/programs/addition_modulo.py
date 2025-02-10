from nada_dsl import *


def nada_main():
    party1 = Party(name="Party1")
    a = SecretUnsignedInteger(Input(name="A", party=party1))
    b = SecretUnsignedInteger(Input(name="B", party=party1))

    result = a + (b % UnsignedInteger(5))

    return [Output(result, "my_output", party1)]
