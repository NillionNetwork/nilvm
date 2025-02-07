from nada_dsl import *


def nada_main():
    dealer1 = Party(name="D1")
    dealer2 = Party(name="D2")
    dealer3 = Party(name="D3")
    result_party = Party(name="R")

    a = SecretUnsignedInteger(Input(name="A", party=dealer1))
    b = SecretUnsignedInteger(Input(name="B", party=dealer2))
    c = SecretUnsignedInteger(Input(name="C", party=dealer3))

    output = a * b * c

    return [Output(output, "output", result_party)]
