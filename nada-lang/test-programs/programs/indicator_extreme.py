from nada_dsl import *


def nada_main():
    party1 = Party(name="Party1")
    value = SecretInteger(Input(name="value", party=party1))

    indicator_large = (value >= Integer(2)).if_else(Integer(1), Integer(0))
    indicator_small = (value <= Integer(-2)).if_else(Integer(1), Integer(0))

    indicator_extreme = indicator_large + indicator_small

    return [Output(indicator_extreme, "indicator_extreme", party=party1)]
