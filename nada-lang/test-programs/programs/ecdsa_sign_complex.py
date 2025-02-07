from nada_dsl import *

def nada_main():
    party1 = Party(name="Party1")
    private_key = EcdsaPrivateKey(Input(name="private_key", party=party1))
    digest = EcdsaDigestMessage(Input(name="digest", party=party1))
    
    new_int = private_key.ecdsa_sign(digest)
    
    a = PublicInteger(Input(name="public_my_int2", party=party1)) 
    b = SecretInteger(Input(name="my_int1", party=party1))
    c = Integer(-42)

    result = a >= (b * c)

    return [
        Output(new_int, "my_output", party1),
        Output(result, "my_output_result", party1)
    ]