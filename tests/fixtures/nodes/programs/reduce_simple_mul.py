from nada_dsl import Array, Party, SecretInteger, Output, Input


def nada_main():
    party1 = Party(name="Party1")
    my_array_1 = Array(SecretInteger(Input(name="my_array_1", party=party1)), size=3)
    my_int = SecretInteger(Input(name="my_int", party=party1))

    def mul(a: SecretInteger, b: SecretInteger) -> SecretInteger:
        return a * b

    multiplication = my_array_1.reduce(mul, my_int)

    out = Output(multiplication, "out", party1)

    return [out]
