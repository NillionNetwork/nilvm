from nada_dsl import *


def nada_main():
    party1 = Party(name="Party1")
    my_int = SecretInteger(Input(name="my_int", party=party1))

    def reuse(values: Tuple[SecretInteger, SecretInteger]) -> SecretInteger:
        c = values.left
        return values.left + c

    my_integer_array = Array.new(my_int, my_int, my_int)

    output = my_integer_array.zip(my_integer_array).map(reuse)

    return [Output(output, "my_output", party1)]
