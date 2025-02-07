from nada_dsl import *

def nada_main():
    party1 = Party(name="party1")

    my_int1 = PublicInteger(Input(name="my_int1", party=party1))
    my_int2 = SecretInteger(Input(name="my_int2", party=party1))

    comp = my_int1 <= Integer(10)
    output = comp.if_else(my_int2, Integer(100))

    return [Output(output, "my_output", party1)]
