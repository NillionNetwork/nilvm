from nada_dsl import *

def nada_main():
    tecdsa_key_party = Party(name="tecdsa_key_party")
    tecdsa_output_party = Party(name="tecdsa_output_party")

    key = EcdsaPrivateKey(Input(name="tecdsa_private_key", party=tecdsa_key_party))
    public_key = key.public_key()

    return [Output(public_key, "tecdsa_public_key", tecdsa_output_party)]