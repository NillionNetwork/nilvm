from nada_dsl import *


def nada_main():
    """Tests compound Nada functions with multiple output"""
    party1 = Party(name="Party1")
    my_int1 = SecretInteger(Input(name="my_int1", party=party1))
    my_int2 = SecretInteger(Input(name="my_int2", party=party1))

    def add(a: SecretInteger, b: SecretInteger) -> SecretInteger:
        return a + b

    def add_times(a: SecretInteger, b: SecretInteger) -> SecretInteger:
        return add(a, b) * add(a, b)

    result1 = add(my_int1, my_int2)
    result2 = add_times(my_int1, my_int2)
    return [
        Output(result1, "my_output1", party1),
        Output(result2, "my_output2", party1),
    ]
