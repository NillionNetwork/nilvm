from nada_dsl import *

def nada_main():
    party1 = Party(name="Party1")
    private_key = EddsaPrivateKey(Input(name="private_key", party=party1))
    message = EddsaMessage(Input(name="message", party=party1))
    
    signature = private_key.eddsa_sign(message)
    return [Output(signature, "signature", party1)]
