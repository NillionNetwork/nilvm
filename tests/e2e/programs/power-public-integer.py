from nada_dsl import *


def nada_main():
    party1 = Party(name="Dealer")
    a = PublicUnsignedInteger(Input(name="A", party=party1))
    b = PublicUnsignedInteger(Input(name="B", party=party1))
    result = a**b
    return [Output(result, "result", party1)]
