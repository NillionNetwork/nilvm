from nada_dsl import *


def nada_main():
    party1 = Party(name="Party1")
    a = SecretInteger(Input(name="a", party=party1))
    b = SecretInteger(Input(name="b", party=party1))

    c = a + b

    my_integer_array = Array.new(c)
    
    return [Output(my_integer_array, "my_output", party1)]
