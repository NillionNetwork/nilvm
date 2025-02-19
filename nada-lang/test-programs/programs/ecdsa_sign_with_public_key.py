from nada_dsl import *

def nada_main():
    tecdsa_key_party = Party(name="tecdsa_key_party")
    tecdsa_digest_message_party = Party(name="tecdsa_digest_message_party")
    tecdsa_output_party = Party(name="tecdsa_output_party")

    key = EcdsaPrivateKey(Input(name="tecdsa_private_key", party=tecdsa_key_party))
    public_key = key.public_key()
    digest = EcdsaDigestMessage(
        Input(name="tecdsa_digest_message", party=tecdsa_digest_message_party)
    )

    signature = key.ecdsa_sign(digest)

    return [
        Output(signature, "tecdsa_signature", tecdsa_output_party),
        Output(digest, "tecdsa_digest_message", tecdsa_output_party),
        Output(public_key, "tecdsa_public_key", tecdsa_output_party),
    ]