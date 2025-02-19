from nada_dsl import *

def nada_main():
    party1 = Party(name="Party1")
    private_key = EddsaPrivateKey(Input(name="private_key", party=party1))
    message = EddsaMessage(Input(name="message", party=party1))
    
    new_int = private_key.eddsa_sign(message)
    return [Output(new_int, "my_output", party1)]