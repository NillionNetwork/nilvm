from nada_dsl import *


def nada_main():
    party1 = Party(name="Party1")
    a = SecretInteger(Input(name="a", party=party1))
    b = SecretInteger(Input(name="b", party=party1))
    c = SecretInteger(Input(name="c", party=party1))
    d = SecretInteger(Input(name="d", party=party1))

    array_a = Array.new(a, b)
    array_b = Array.new(c, d)

    my_integer_array = Array.new(array_a, array_b)
    
    return [Output(my_integer_array, "my_output", party1)]
