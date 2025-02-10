from nada_dsl import *


def nada_main():
    party1 = Party(name="Party1")
    i00 = Array(Array(SecretInteger(Input(name="I00", party=party1)), size=2), size=3)
    return [Output(i00, "my_output", party1)]
