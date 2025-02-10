from nada_dsl import *


def nada_main():
    party1 = Party(name="Party1")
    my_int1 = SecretInteger(Input(name="my_int1", party=party1))
    my_int2 = SecretInteger(Input(name="my_int2", party=party1))

    mid = my_int1 + Integer(2) * my_int2 + Integer(4)

    new_int = mid % my_int2

    output = new_int + my_int1

    return [Output(output, "my_output", party1)]
