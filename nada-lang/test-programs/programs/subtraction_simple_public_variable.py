from nada_dsl import *


def nada_main():
    party1 = Party(name="Party1")
    A = SecretInteger(Input(name="A", party=party1))
    B = PublicInteger(Input(name="B", party=party1))

    new_int = A - B

    return [Output(new_int, "my_output", party1)]
