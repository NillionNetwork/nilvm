from nada_dsl import *


def nada_main():
    dealer = Party(name="Dealer")
    result = Party(name="Result")

    A = PublicInteger(Input(name="A", party=dealer))
    B = PublicInteger(Input(name="B", party=dealer))
    C = PublicInteger(Input(name="C", party=dealer))
    D = PublicInteger(Input(name="D", party=dealer))

    TMP0 = A + B  # public + public
    TMP1 = C * D  # public * public
    TMP2 = B + D  # public + public
    TMP3 = B * D  # public * public

    O = TMP0 + TMP1 + TMP2 + TMP3

    return [Output(O, "O", result)]
