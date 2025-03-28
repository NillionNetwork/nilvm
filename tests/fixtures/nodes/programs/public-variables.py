from nada_dsl import *


def nada_main():
    dealer = Party(name="Dealer")
    result = Party(name="Result")

    I00 = SecretInteger(Input(name="I00", party=dealer))
    I01 = PublicInteger(Input(name="I01", party=dealer))

    Add0 = I00 + I01

    return [Output(Add0, "Add0", result)]
