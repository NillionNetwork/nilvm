from nada_dsl import *


def nada_main():
    party1 = Party(name="Party1")
    a = SecretInteger(Input(name="a", party=party1))
    b = SecretInteger(Input(name="b", party=party1))

    def add(left: SecretInteger, right: SecretInteger) -> SecretInteger:
        return left + right

    def map_add(values: Tuple[SecretInteger, SecretInteger]) -> SecretInteger:
        return values.left + values.right

    value = add(a, b)
    my_integer_array = Array.new(value, value, value)

    my_integer_array = my_integer_array.zip(my_integer_array).map(map_add)

    return [Output(my_integer_array, "my_output", party1)]
