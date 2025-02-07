from nada_dsl import *


def nada_main():
    party1 = Party(name="Party1")
    i00 = SecretInteger(Input(name="I00", party=party1))
    i01 = SecretInteger(Input(name="I01", party=party1))
    res = Array.new(i00, i01)
    return [Output(res, "my_output", party1)]
