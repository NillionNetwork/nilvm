# from nada_dsl import Party, Input, Output, EddsaPrivateKey, EddsaMessage
from nada_dsl import *

def nada_main():
    teddsa_key_party = Party(name="teddsa_key_party")
    teddsa_message_party = Party(name="teddsa_message_party")
    teddsa_output_party = Party(name="teddsa_output_party")

    key = EddsaPrivateKey(Input(name="teddsa_private_key", party=teddsa_key_party))
    public_key = key.public_key()
    message = EddsaMessage(
        Input(name="teddsa_message", party=teddsa_message_party)
    )

    signature = key.eddsa_sign(message)

    return [
        Output(signature, "teddsa_signature", teddsa_output_party),
        Output(message, "teddsa_message", teddsa_output_party),
        Output(public_key, "teddsa_public_key", teddsa_output_party),
    ]
