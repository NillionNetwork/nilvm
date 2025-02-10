from nada_dsl import *


def nada_main():
    party1 = Party(name="Party1")
    my_array_1 = Array(SecretInteger(Input(name="my_array_1", party=party1)), size=3)
    my_array_2 = Array(SecretInteger(Input(name="my_array_2", party=party1)), size=3)
    my_array_3 = Array(SecretInteger(Input(name="my_array_3", party=party1)), size=3)

    new_array = my_array_1.zip(my_array_2).zip(my_array_3)

    out = Output(new_array, "my_output", party1)

    return [out]
