from nada_dsl import *


def nada_main():
    party1 = Party(name="Dealer")
    a = UnsignedInteger(33)
    b = PublicUnsignedInteger(Input(name="my_uint1_public", party=party1))

    result = a % b
    return [Output(result, "my_output", party1)]
