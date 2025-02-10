from nada_dsl import *


def nada_main():
    dealer = Party(name="Dealer")
    result = Party(name="Result")

    I00 = SecretUnsignedInteger(Input(name="I00", party=dealer))
    I01 = PublicUnsignedInteger(Input(name="I01", party=dealer))
    I02 = PublicUnsignedInteger(Input(name="I02", party=dealer))
    I03 = SecretUnsignedInteger(Input(name="I03", party=dealer))
    I04 = PublicUnsignedInteger(Input(name="I04", party=dealer))

    Mul0 = I00 * I01
    Mul1 = Mul0 * I02
    Mul2 = I03 * I04
    Sub0 = Mul1 - Mul2

    return [Output(Sub0, "Sub0", result)]
