import os
import sys

sys.path.insert(
    0,
    os.path.join(os.path.dirname(os.path.realpath(__file__)), f"./lib/"),
)

from library import add
from nada_dsl import *


def nada_main():
    party1 = Party(name="Dealer")
    a = SecretInteger(Input(name="A", party=party1))
    b = SecretInteger(Input(name="B", party=party1))

    result = add(a, b)

    return [Output(result, "result", party1)]
