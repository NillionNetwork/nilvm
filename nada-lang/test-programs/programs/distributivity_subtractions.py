from nada_dsl import *


def nada_main():
    party1 = Party(name="Party1")
    x = SecretInteger(Input(name="x", party=party1))
    y = SecretInteger(Input(name="y", party=party1))
    z = SecretInteger(Input(name="z", party=party1))

    out_1 = x - y + z
    out_2 = x + y - z
    out_3 = x - (y + z)
    out_4 = x * (x - (y + z))
    out_5 = x - (y + z) + x
    out_6 = x + y - x * y

    return [Output(out_1, "out_1", party1),
            Output(out_2, "out_2", party1),
            Output(out_3, "out_3", party1),
            Output(out_4, "out_4", party1),
            Output(out_5, "out_5", party1),
            Output(out_6, "out_6", party1)]
