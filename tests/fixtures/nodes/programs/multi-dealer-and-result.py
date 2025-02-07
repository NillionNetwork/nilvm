from nada_dsl import *
from nada_dsl.future import *


def nada_main():
    input_party1 = Party(name="I1")
    input_party2 = Party(name="I2")
    a = SecretInteger(Input(name="A", party=input_party1))
    b = SecretInteger(Input(name="B", party=input_party2))

    product = a * b
    addition = a + b

    output_party1 = Party(name="O1")
    output_party2 = Party(name="O2")

    return [
        Output(product, "product", output_party1),
        Output(addition, "addition", output_party2),
    ]
