from nada_dsl import Party, Input, Output, EcdsaPrivateKey, EcdsaDigestMessage


def nada_main():
    tecdsa_key_party = Party(name="tecdsa_key_party")
    tecdsa_digest_message_party = Party(name="tecdsa_digest_message_party")
    tecdsa_output_party = Party(name="tecdsa_output_party")

    key = EcdsaPrivateKey(Input(name="tecdsa_private_key", party=tecdsa_key_party))
    digest = EcdsaDigestMessage(
        Input(name="tecdsa_digest_message", party=tecdsa_digest_message_party)
    )

    signature = key.ecdsa_sign(digest)

    return [Output(signature, "tecdsa_signature", tecdsa_output_party)]
