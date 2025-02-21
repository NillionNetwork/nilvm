from nada_dsl import *

def nada_main():
    teddsa_key_party = Party(name="teddsa_key_party")
    teddsa_output_party = Party(name="teddsa_output_party")

    key = EddsaPrivateKey(Input(name="teddsa_private_key", party=teddsa_key_party))
    public_key = key.public_key()

    return [Output(public_key, "teddsa_public_key", teddsa_output_party)]