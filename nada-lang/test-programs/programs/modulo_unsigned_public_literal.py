from nada_dsl import *

def nada_main():
    party1 = Party(name="Dealer")
    a = PublicUnsignedInteger(Input(name="my_uint1", party=party1) )
    b = UnsignedInteger(33)

    result = a % b
    return [Output(result, "my_output", party1)]
