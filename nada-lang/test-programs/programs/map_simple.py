from nada_dsl import *


def nada_main():
    party1 = Party(name="Party1")
    my_array_1 = Array(SecretInteger(Input(name="my_array_1", party=party1)), size=3)
    my_int = SecretInteger(Input(name="my_int", party=party1))

    def inc(a: SecretInteger) -> SecretInteger:
        return a + my_int

    new_array = my_array_1.map(inc)

    out = Output(new_array, "my_output", party1)

    return [out]
