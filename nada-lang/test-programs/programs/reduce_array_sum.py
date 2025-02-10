from nada_dsl import *


def nada_main():
    party1 = Party(name="Party1")
    my_array = Array(SecretInteger(Input(name="my_array", party=party1)), size=4)
    secret_int0 = SecretInteger(Input(name="secret_int0", party=party1))

    def add(a: SecretInteger, b: SecretInteger) -> SecretInteger:
        return a + b

    sum = my_array.reduce(add, secret_int0)

    out = Output(sum, "my_output", party1)

    return [out]
