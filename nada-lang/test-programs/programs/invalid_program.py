"""This is an example program to test invalidation of programs
by the ProgramAuditor"""

from nada_dsl import *


def nada_main():
    party1 = Party(name="Party1")
    my_array_1 = Array(SecretInteger(Input(name="my_array_1", party=party1)), size=1001)
    my_int = SecretInteger(Input(name="my_int", party=party1))

    def div(a: SecretInteger) -> SecretInteger:
        return a / my_int

    new_array = my_array_1.map(div)
    out = Output(new_array, "out", party1)
    return [out]
