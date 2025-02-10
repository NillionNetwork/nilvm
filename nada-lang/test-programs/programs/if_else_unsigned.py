from nada_dsl import *


def nada_main():
    party1 = Party(name="Party1")
    party2 = Party(name="Party2")
    my_uint1_secret = SecretUnsignedInteger(Input(name="my_uint1_secret", party=party1))
    my_uint2_secret = SecretUnsignedInteger(Input(name="my_uint2_secret", party=party2))

    cond = my_uint1_secret < my_uint2_secret
    output = cond.if_else(my_uint1_secret, my_uint2_secret)

    return [Output(output, "my_output", party1)]
