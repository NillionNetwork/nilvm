from nada_dsl import *

def nada_main():
    party1 = Party(name="Party1")
    private_key = EcdsaPrivateKey(Input(name="private_key", party=party1))
    digest = EcdsaDigestMessage(Input(name="digest", party=party1))
    
    new_int = private_key.ecdsa_sign(digest)
    return [Output(new_int, "my_output", party1)]