from nada_dsl import *

def nada_main():
    teddsa_key_party = Party(name="teddsa_key_party")
    teddsa_message_party = Party(name="teddsa_message_party")
    teddsa_output_party = Party(name="teddsa_output_party")

    key = EddsaPrivateKey(Input(name="teddsa_private_key", party=teddsa_key_party))
    message = EddsaMessage(Input(name="teddsa_message", party=teddsa_message_party))

    signature = key.eddsa_sign(message)

    return [Output(signature, "teddsa_signature", teddsa_output_party)]
