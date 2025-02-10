from nada_dsl import *


def nada_main():
    party1 = Party(name="Party1")
    a = SecretInteger(Input(name="a", party=party1))
    b = SecretInteger(Input(name="b", party=party1))
    c = SecretInteger(Input(name="c", party=party1))

    d = a + b

    my_tuple = Tuple.new(c, d)
    
    return [Output(my_tuple, "my_output", party1)]
