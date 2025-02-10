from nada_dsl import Array, Party, SecretInteger, Output, Input, Integer


def nada_main():
    party1 = Party(name="Party1")
    my_array_1 = Array(SecretInteger(Input(name="my_array_1", party=party1)), size=3)
    secret_int0 = SecretInteger(Input(name="secret_int0", party=party1))

    def add(a: SecretInteger, b: SecretInteger) -> SecretInteger:
        return a + b

    addition = my_array_1.reduce(add, secret_int0)

    out = Output(addition, "my_output", party1)

    return [out]
