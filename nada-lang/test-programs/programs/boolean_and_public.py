from nada_dsl import *
from nada_dsl.future import *


def nada_main():
    party1 = Party(name="Party1")
    party2 = Party(name="Party2")
    A = PublicInteger(Input(name="public_A", party=party1))
    B = PublicInteger(Input(name="public_B", party=party2))
    C = PublicInteger(Input(name="public_C", party=party2))

    result = (A < (B + C)) & (A < C)

    return [Output(result, "my_output", party1)]
