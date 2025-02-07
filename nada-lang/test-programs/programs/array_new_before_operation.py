from nada_dsl import *


def nada_main():
    party1 = Party(name="Party1")
    a = SecretInteger(Input(name="a", party=party1))
    b = SecretInteger(Input(name="b", party=party1))
    my_int = SecretInteger(Input(name="my_int", party=party1))

    my_integer_array = Array.new(a, b)

    def add(a: SecretInteger, b: SecretInteger) -> SecretInteger:
        return a + b

    addition = my_integer_array.reduce(add, my_int)

    
    return [Output(addition, "my_output", party1)]
