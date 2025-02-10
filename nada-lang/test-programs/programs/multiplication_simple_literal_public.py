from nada_dsl import *


def nada_main():
    party1 = Party(name="Party1")
    my_int1 = PublicInteger(Input(name="my_int1", party=party1))

    new_int = Integer(13) * my_int1

    return [Output(new_int, "my_output", party1)]
