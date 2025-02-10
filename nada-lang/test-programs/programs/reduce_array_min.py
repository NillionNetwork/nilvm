from nada_dsl import *


def nada_main():
    party1 = Party(name="Party1")
    my_array = Array(SecretInteger(Input(name="my_array", party=party1)), size=4)
    secret_int100000 = SecretInteger(Input(name="secret_int100000", party=party1))

    def max(a: SecretInteger, b: SecretInteger) -> SecretInteger:
        return (a < b).if_else(a, b)

    addition = my_array.reduce(max, secret_int100000)

    out = Output(addition, "my_output", party1)

    return [out]
