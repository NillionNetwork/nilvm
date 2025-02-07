from nada_dsl import *


def nada_main():
    party1 = Party(name="Party1")
    party2 = Party(name="Party2")
    my_int1 = PublicInteger(Input(name="my_int1", party=party1))
    my_int2 = SecretInteger(Input(name="my_int2", party=party2))

    min = Integer(0)
    min = (my_int1 < min).if_else(my_int1, min)
    min = (my_int2 < min).if_else(my_int2, min)
    return [Output(min, "my_output", party1)]
