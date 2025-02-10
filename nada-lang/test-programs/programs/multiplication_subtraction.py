from nada_dsl import *


def nada_main():
    party1 = Party(name="Party1")
    a = SecretInteger(Input(name="A", party=party1))
    b = SecretInteger(Input(name="B", party=party1))

    result = Integer(1) * (a - b)

    return [Output(result, "my_output", party1)]
