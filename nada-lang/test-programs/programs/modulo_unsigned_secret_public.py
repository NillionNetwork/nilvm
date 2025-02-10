from nada_dsl import *


def nada_main():
    party1 = Party(name="Party1")
    my_int1 = SecretUnsignedInteger(Input(name="my_uint1", party=party1))
    public_my_int2 = PublicUnsignedInteger(Input(name="my_uint2_public", party=party1))

    new_int = my_int1 % public_my_int2

    return [Output(new_int, "my_output", party1)]
