from nada_dsl import *

def nada_main():
    party1 = Party(name="Party1")
    secret_int1 = SecretInteger(Input(name="secret_int1", party=party1))
    secret_int2 = SecretInteger(Input(name="secret_int2", party=party1))

    result = secret_int1 + secret_int2

    return [Output(result, "result", party1)]
