from nada_dsl import *


def nada_main():
    party1 = Party(name="Party1")

    I00 = SecretUnsignedInteger(Input(name="I00", party=party1))
    A00 = Array(SecretInteger(Input(name="A00", party=party1)), size=5)

    res = Tuple.new(I00, A00)

    return [Output(res, "my_output", party1)]
