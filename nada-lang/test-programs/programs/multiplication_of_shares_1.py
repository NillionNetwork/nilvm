from nada_dsl import *


def nada_main():
    party1 = Party(name="Party1")
    a = SecretInteger(Input(name="A", party=party1))
    b = SecretInteger(Input(name="B", party=party1))
    c = SecretInteger(Input(name="C", party=party1))

    result = (a + b) * c

    return [Output(result, "my_output", party1)]
