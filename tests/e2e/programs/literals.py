from nada_dsl import *


def nada_main():
    dealer = Party(name="Dealer")
    result = Party(name="Result")

    I00 = SecretInteger(Input(name="I00", party=dealer))

    TMP0 = I00 * Integer(13) + Integer(13)    # secret * literal + literal (checks literal re-use)
    TMP1 = I00 * Integer(50)                  # secret * literal
    TMP2 = Integer(13) + Integer(50)          # literal + literal
    TMP3 = Integer(13) * Integer(50)          # literal * literal

    Add0 = TMP0 + TMP1 + TMP2 + TMP3

    return [Output(Add0, "Add0", result)]
