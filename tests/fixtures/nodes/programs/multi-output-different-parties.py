from nada_dsl import *


def nada_main():
    party_1 = Party(name="p1")
    party_2 = Party(name="p2")

    i = SecretUnsignedInteger(Input(name="i", party=party_1))
    o = i + i
    return [Output(i, "o1", party_1), Output(o, "o2", party_2)]

