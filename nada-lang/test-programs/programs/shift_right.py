from nada_dsl import *


def nada_main():
    party1 = Party(name="Party1")
    my_int1 = SecretInteger(Input(name="my_int1", party=party1))
    amount = PublicUnsignedInteger(Input(name="amount", party=party1))

    new_int = my_int1 >> amount

    return [Output(new_int, "my_output", party1)]
