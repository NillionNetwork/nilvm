from nada_dsl import *


def nada_main():
    party1 = Party(name="Party1")
    my_int1 = SecretInteger(Input(name="my_int1", party=party1))
    my_int2 = SecretInteger(Input(name="my_int2", party=party1))
    my_int3 = SecretInteger(Input(name="my_int3", party=party1))

    comp = my_int1 < my_int2

    result = comp.if_else(my_int1, my_int2)

    mid = result % my_int3

    mid2 = mid + Integer(3)

    output = mid2 >= my_int1

    return [Output(output, "my_output", party1)]
