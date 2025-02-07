from nada_dsl import *

def nada_main():
    party = Party(name="Party1")
    random_bool = SecretBoolean.random()

    return [Output(random_bool, "my_output", party)]
