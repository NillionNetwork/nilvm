from nada_dsl import *
from nada_dsl.future import *


def nada_main():
    party1 = Party(name="Party1")

    result = Integer(15) <= Integer(59)

    return [Output(result, "my_output", party1)]
