from nada_dsl import *


def nada_main():
    party1 = Party(name="Party1")
    my_uint1 = SecretUnsignedInteger(Input(name="my_uint1", party=party1))

    new_int = my_uint1 >> UnsignedInteger(2)

    return [Output(new_int, "my_output", party1)]
