# Program to multiply two square matrices using nested loops
# This is much more inefficient than using NumPy.

from nada_dsl import *

dimension = 100


def nada_main():
    # Create inputs
    party1 = Party(name="Party1")
    A = [
        [
            SecretInteger(Input(name=f"A_{i}_{j}", party=party1))
            for i in range(dimension)
        ]
        for j in range(dimension)
    ]
    B = [
        [
            SecretInteger(Input(name=f"B_{i}_{j}", party=party1))
            for i in range(dimension)
        ]
        for j in range(dimension)
    ]

    result = [[Integer(0) for i in range(dimension)] for j in range(dimension)]

    for i in range(dimension):
        for j in range(dimension):
            for k in range(dimension):
                result[i][j] += A[i][k] + B[k][j]

    output_list = [
        Output(result[i][j], name=f"result_{i}_{j}", party=party1)
        for i in range(dimension)
        for j in range(dimension)
    ]

    return output_list
