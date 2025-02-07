from nada_dsl import *


def nada_main():
    party1 = Party(name="Party1")
    i00 = Array(SecretInteger(Input(name="I00", party=party1)), size=10)
    return [Output(i00, "my_output", party1)]
