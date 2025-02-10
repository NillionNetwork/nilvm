from nada_dsl import *

num_mults = 1000000


def nada_main():
    party1 = Party(name="Party1")
    my_int1 = SecretInteger(Input(name="my_int1", party=party1))
    my_int2 = SecretInteger(Input(name="my_int2", party=party1))
    my_int3 = SecretInteger(Input(name="my_int3", party=party1))
    my_int4 = SecretInteger(Input(name="my_int4", party=party1))

    input_list = [my_int1, my_int2, my_int3, my_int4]

    product_value = my_int1
    index = 0
    for _ in range(num_mults):
        product_value = product_value * input_list[index]
        index += 1
        if index == 4:
            index = 0

    return [Output(product_value, "my_output", party1)]
