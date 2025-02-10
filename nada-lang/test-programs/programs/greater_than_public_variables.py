import os
import sys

sys.path.insert(
    0, os.path.join(os.path.dirname(os.path.realpath(__file__)), f"../../../../")
)

from nada_dsl import *


def nada_main():
    party1 = Party(name="Party1")
    party2 = Party(name="Party2")
    A = PublicInteger(Input(name="public_A", party=party1))
    B = PublicInteger(Input(name="public_B", party=party2))
    C = PublicInteger(Input(name="public_C", party=party1))
    D = PublicInteger(Input(name="public_D", party=party2))

    result = A * B + C > B * D

    return [Output(result, "my_output", party1)]
