from nada_dsl import *


def nada_main():
    party1 = Party(name="Party1")
    a = PublicUnsignedInteger(Input(name="A", party=party1))
    b = PublicUnsignedInteger(Input(name="B", party=party1))
    c = PublicUnsignedInteger(Input(name="C", party=party1))
    d = PublicUnsignedInteger(Input(name="D", party=party1))

    result = (a + b) * (b / UnsignedInteger(2)) * (c ** d) * (a % UnsignedInteger(5))

    return [Output(result, "my_output", party1)]
