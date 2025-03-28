from nada_dsl import *

num_secret_multiplications = 10000
num_public_multiplications = 10000
num_additions = 10000
num_divisions = 3
num_comparisons = 10
num_private_equalities = 10


def nada_main():
    party1 = Party(name="Party1")
    secret_int1 = SecretInteger(Input(name="secret_int1", party=party1))
    secret_int2 = SecretInteger(Input(name="secret_int2", party=party1))
    public_int = PublicInteger(Input(name="public_int", party=party1))

    # multiplications
    result = secret_int1
    for _ in range(num_secret_multiplications):
        result = result * secret_int2

    # public mult
    for _ in range(num_public_multiplications):
        result = result * public_int

    # additions
    for _ in range(num_additions):
        result = result + secret_int2

    # divisions
    for _ in range(num_divisions):
        result = result / secret_int2

    return [Output(result, "result", party1)]
