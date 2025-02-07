from nada_dsl import *

'''
Note: Although the input types of `x` and `y` are `PublicUnsignedInteger`, we
assume that the inputs are 0 or 1 so we do not use any modulo 2 operations.
'''


def AND_gate(x: PublicUnsignedInteger, y: PublicUnsignedInteger) -> PublicUnsignedInteger:
    return (x * y)


def OR_gate(x: PublicUnsignedInteger, y: PublicUnsignedInteger) -> PublicUnsignedInteger:
    return x + y - x * y


def XOR_gate(x: PublicUnsignedInteger, y: PublicUnsignedInteger) -> PublicUnsignedInteger:
    return x + y - UnsignedInteger(2) * x * y


def NOT_gate(x: PublicUnsignedInteger) -> PublicUnsignedInteger:
    return (UnsignedInteger(1) - x)


def NAND_gate(x: PublicUnsignedInteger, y: PublicUnsignedInteger) -> PublicUnsignedInteger:
    return NOT_gate(AND_gate(x, y))


def NOR_gate(x: PublicUnsignedInteger, y: PublicUnsignedInteger) -> PublicUnsignedInteger:
    return NOT_gate(OR_gate(x, y))


def XNOR_gate(x: PublicUnsignedInteger, y: PublicUnsignedInteger) -> PublicUnsignedInteger:
    return NOT_gate(XOR_gate(x, y))


def nada_main():
    party1 = Party(name="Party1")

    x = PublicUnsignedInteger(Input(name="x", party=party1))
    y = PublicUnsignedInteger(Input(name="y", party=party1))

    and_gate = AND_gate(x, y)
    or_gate = OR_gate(x, y)
    xor_gate = XOR_gate(x, y)
    not_gate = NOT_gate(x)
    nand_gate = NAND_gate(x, y)
    nor_gate = NOR_gate(x, y)
    xnor_gate = XNOR_gate(x, y)

    return [
        Output(and_gate, "and_gate", party1),
        Output(or_gate, "or_gate", party1),
        Output(xor_gate, "xor_gate", party1),
        Output(not_gate, "not_gate", party1),
        Output(nand_gate, "nand_gate", party1),
        Output(nor_gate, "nor_gate", party1),
        Output(xnor_gate, "xnor_gate", party1),
    ]
