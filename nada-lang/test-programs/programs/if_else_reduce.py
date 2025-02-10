from nada_dsl import *


def nada_main():
    party1 = Party(name="Party1")
    party2 = Party(name="Party2")

    my_array = Array(SecretInteger(Input(name="my_array", party=party1)), size=3)
    public_int0 = PublicInteger(Input(name="public_int0", party=party1))

    def count_negative_numbers(a: PublicInteger, b: SecretInteger) -> PublicInteger:
        cond = b >= Integer(0)
        is_negative_number = cond.to_public().if_else(Integer(0), Integer(1))
        return a + is_negative_number

    output = my_array.reduce(count_negative_numbers, public_int0)

    return [Output(output, "my_output", party1)]
