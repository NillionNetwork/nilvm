from nada_dsl import *

def nada_main():
    party = Party(name="Party1")
    my_int = SecretInteger.random()

    return [Output(my_int, "my_output", party)]