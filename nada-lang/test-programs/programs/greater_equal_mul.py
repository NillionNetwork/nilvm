from nada_dsl import *


def nada_main():
    party1 = Party(name="Dealer")
    a = PublicInteger(Input(name="public_my_int2", party=party1))  # 81
    b = SecretInteger(Input(name="my_int1", party=party1))  # 32
    c = Integer(-42)

    result = a >= (b * c)
    return [Output(result, "my_output", party1)]
