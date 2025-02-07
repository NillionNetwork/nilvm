from nada_dsl import *


def nada_main():
    party1 = Party(name="Party1")
    new_int = Integer(13) * Integer(13)

    return [Output(new_int, "my_output", party1)]
