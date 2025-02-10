from nada_dsl import *


def nada_main():
    party1 = Party(name="Party1")
    my_array_1 = Array(SecretInteger(Input(name="my_array_1", party=party1)), size=3)
    my_array_2 = Array(PublicInteger(Input(name="my_array_2", party=party1)), size=3)

    inner_product = my_array_1.inner_product(my_array_2)

    out = Output(inner_product, "out", party1)

    return [out]
