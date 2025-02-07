from nada_dsl import *

def nada_main():
    party1 = Party(name="party1")

    my_int1 = SecretInteger(Input(name="my_int1", party=party1))

    comp = my_int1 <= Integer(10)
    output = comp.if_else(Integer(1), Integer(2))

    return [Output(output, "my_output", party1)]
