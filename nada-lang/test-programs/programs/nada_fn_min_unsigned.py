from nada_dsl import *


def nada_main():
    party1 = Party(name="Party1")
    my_int1 = SecretUnsignedInteger(Input(name="my_int1", party=party1))
    my_int2 = SecretUnsignedInteger(Input(name="my_int2", party=party1))

    def min(a: SecretUnsignedInteger, b: SecretUnsignedInteger) -> SecretUnsignedInteger:
        return (a < b).if_else(a, b)

    new_int = min(my_int1, my_int2)
    return [Output(new_int, "my_output", party1)]
