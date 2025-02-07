from nada_dsl import *


def nada_main():
    party1 = Party(name="Party1")
    a = SecretInteger(Input(name="a", party=party1))
    b = SecretInteger(Input(name="b", party=party1))
    c = SecretUnsignedInteger(Input(name="c", party=party1))
    d = SecretUnsignedInteger(Input(name="d", party=party1))

    left = (c < d).if_else(UnsignedInteger(42), d)
    right = (a * b).to_public() * Integer(10)
    result = Tuple.new(left, right)

    return [Output(result, "my_output", party1)]
