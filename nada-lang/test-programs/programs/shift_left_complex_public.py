from nada_dsl import *


def nada_main():
    party1 = Party(name="Party1")
    my_int1 = PublicInteger(Input(name="my_int1", party=party1))
    my_int2 = PublicInteger(Input(name="my_int2", party=party1))

    my_int_shifted = my_int1 << UnsignedInteger(1)
    out_1 = my_int2 + my_int_shifted
    out_2 = my_int2 * my_int_shifted
    out_3 = (my_int2 + my_int2) * my_int_shifted

    return [
        Output(out_1, "out_1", party1),
        Output(out_2, "out_2", party1),
        Output(out_3, "out_3", party1)
    ]
