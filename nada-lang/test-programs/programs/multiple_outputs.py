from nada_dsl import *


def nada_main():
    party1 = Party(name="Party1")
    i00 = Array(SecretInteger(Input(name="I00", party=party1)), size=3)
    i01 = SecretInteger(Input(name="I01", party=party1))
    i02 = Array(SecretInteger(Input(name="I02", party=party1)), size=3)
    i03 = Array(SecretInteger(Input(name="I03", party=party1)), size=3)
    i04 = SecretInteger(Input(name="I04", party=party1))
    return [
        Output(i00, "output_00", party1),
        Output(i01, "output_01", party1),
        Output(i02, "output_02", party1),
        Output(i03, "output_03", party1),
        Output(i04, "output_04", party1),
    ]
