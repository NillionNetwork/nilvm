from nada_dsl import *


def nada_main():
    party1 = Party(name="Party1")
    party2 = Party(name="Party2")

    my_int1 = SecretInteger(Input(name="my_int1", party=party1))
    my_int2 = SecretInteger(Input(name="my_int2", party=party2))

    cond = my_int1 < my_int2

    my_array1 = Array.new(my_int1, my_int2, my_int1)
    my_array2 = Array.new(my_int2, my_int1, my_int1)

    def if_else_and_add(values: Tuple[SecretInteger, SecretInteger]) -> SecretInteger:
        return cond.if_else(values.left, values.right) + Integer(4)

    output = my_array1.zip(my_array2).map(if_else_and_add)

    return [Output(output, "my_output", party1)]
