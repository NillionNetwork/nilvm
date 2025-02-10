from nada_dsl import *


def nada_main():
    party1 = Party(name="party1")

    my_int1 = PublicInteger(Input(name="public_my_int1", party=party1))
    my_int2 = SecretInteger(Input(name="my_int2", party=party1))
    my_int3 = SecretInteger(Input(name="my_int3", party=party1))

    comp = my_int1 <= Integer(10)
    output = comp.if_else(my_int2, my_int3)

    return [Output(output, "my_output", party1)]
