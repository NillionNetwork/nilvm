from nada_dsl import *
from nada_dsl.future import *


def nada_main():
    party1 = Party(name="Party1")
    party2 = Party(name="Party2")
    A = SecretInteger(Input(name="A_neg", party=party1))
    B = SecretInteger(Input(name="B_neg", party=party2))

    result = A < B

    return [Output(result, "my_output", party1)]
