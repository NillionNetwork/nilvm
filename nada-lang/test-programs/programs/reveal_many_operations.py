from nada_dsl import *


def nada_main():
    party1 = Party(name="Party1")
    party2 = Party(name="Party2")
    my_int1 = SecretInteger(Input(name="my_int1", party=party1))
    my_int2 = SecretInteger(Input(name="my_int2", party=party2))

    prod = my_int1 * my_int2
    sum = my_int1 + my_int2
    mod = my_int1 % Integer(3)
    tmp_1 = prod.to_public() / Integer(2)
    tmp_2 = sum.to_public() + mod.to_public()
    output = tmp_1 + tmp_2

    return [Output(output, "my_output", party1)]
