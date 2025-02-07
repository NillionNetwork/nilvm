from nada_dsl import *


def nada_main():
    party1 = Party(name="Party1")
    my_uint_secret = SecretUnsignedInteger(Input(name="my_uint_secret", party=party1))
    my_uint1 = PublicUnsignedInteger(Input(name="my_uint1_public", party=party1))
    my_uint2 = PublicUnsignedInteger(Input(name="my_uint2_public", party=party1))

    cond = my_uint_secret < my_uint2
    output = cond.if_else(my_uint1, my_uint2)

    return [Output(output, "my_output", party1)]
