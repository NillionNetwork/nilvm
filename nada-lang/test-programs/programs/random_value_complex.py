from nada_dsl import *

def nada_main():
    party = Party(name="Party1")
    my_int = SecretInteger(Input(name="my_int1", party=party))
    my_random = SecretInteger.random()

    output = my_int + my_random

    return [Output(output, "my_output", party)]