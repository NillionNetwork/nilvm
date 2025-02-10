from nada_dsl import *


def nada_main():
    party1 = Party(name="Party1")
    a = PublicUnsignedInteger(Input(name="a", party=party1))
    b = PublicUnsignedInteger(Input(name="b", party=party1))

    result = a ** b

    return [Output(result, "my_output", party1)]
