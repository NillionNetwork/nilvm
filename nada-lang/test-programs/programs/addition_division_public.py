from nada_dsl import *

def nada_main():
    party1 = Party(name="Party1")
    a = Integer(-42)
    b = Integer(-2)
    c = PublicInteger(Input(name="C", party=party1) )

    result = a + (b / c)
    return [Output(result, "my_output", party1)]