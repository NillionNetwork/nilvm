from nada_dsl import *


def nada_main():
    party1 = Party(name="Party1")
    my_array_1 = Array(SecretInteger(Input(name="my_array_1", party=party1)), size=3)
    my_array_2 = Array(SecretInteger(Input(name="my_array_2", party=party1)), size=3)
    secret_int0 = SecretInteger(Input(name="secret_int0", party=party1))

    def array_product(values: Tuple[SecretInteger, SecretInteger]) -> SecretInteger:
        return values.left * values.right

    def add(a: SecretInteger, b: SecretInteger) -> SecretInteger:
        return a + b

    new_array = my_array_1.zip(my_array_2).map(array_product)

    inner_product = new_array.reduce(add, secret_int0)

    out = Output(inner_product, "out", party1)

    return [out]
