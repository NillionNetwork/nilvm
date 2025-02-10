from nada_dsl import *


def nada_main():
    party1 = Party(name="Party1")
    my_int = SecretInteger(Input(name="my_int", party=party1))
    my_int1 = PublicInteger(Input(name="public_my_int1", party=party1))
    my_int2 = PublicInteger(Input(name="public_my_int2", party=party1))

    cond = my_int < my_int2
    output = cond.if_else(my_int1, my_int2)

    return [Output(output, "my_output", party1)]
