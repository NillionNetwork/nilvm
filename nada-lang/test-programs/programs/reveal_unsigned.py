from nada_dsl import *


def nada_main():
    party1 = Party(name="Party1")
    party2 = Party(name="Party2")
    my_uint1_secret = SecretUnsignedInteger(Input(name="my_uint1_secret", party=party1))
    my_uint2_secret = SecretUnsignedInteger(Input(name="my_uint2_secret", party=party2))

    x = my_uint1_secret * my_uint2_secret
    output = x.to_public() * UnsignedInteger(3)

    return [Output(output, "my_output", party1)]
