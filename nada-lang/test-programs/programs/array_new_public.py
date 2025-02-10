from nada_dsl import *


def nada_main():
    party1 = Party(name="Party1")
    a = PublicInteger(Input(name="a", party=party1))
    b = PublicInteger(Input(name="b", party=party1))

    my_integer_array = Array.new(a, b)
    
    return [Output(my_integer_array, "my_output", party1)]
