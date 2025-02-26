use crate::vm::tests::simulate;
use anyhow::{anyhow, Error, Ok};
use cggmp21::generic_ec::{curves::Ed25519, NonZero, SecretScalar};
use execution_engine_vm::simulator::inputs::StaticInputGeneratorBuilder;
use givre::{signing::aggregate::Signature, Ciphersuite};
use nada_value::NadaValue;
use threshold_keypair::{privatekey::ThresholdPrivateKey, publickey::ThresholdPublicKey, signature::EddsaSignature};

fn verify(pk: ThresholdPublicKey<Ed25519>, signature: EddsaSignature, message: &[u8]) -> bool {
    let givre_sig = Signature { r: signature.signature.r, z: signature.signature.z };
    let pk_point = NonZero::from_point(*pk.as_point()).expect("Public key should not be zero!");

    // Normalize the point using givre's normalize_point
    let pk_normalized = Ciphersuite::normalize_point(pk_point); // NormalizedPoint<_, Point<Ed25519>>

    // Call verify with the normalized and non-zero point
    givre_sig.verify(&pk_normalized, &message).is_ok()
}

// Since this is probabilistic, we're checking if the signature is valid
#[test]
fn eddsa_sign() -> Result<(), Error> {
    let sk = ThresholdPrivateKey::<Ed25519>::from_scalar(SecretScalar::random(&mut rand::thread_rng())).unwrap();
    let message = b"Some message to be signed";

    let inputs = StaticInputGeneratorBuilder::default()
        .add_eddsa_message("teddsa_message", message.to_vec())
        .add_eddsa_private_key("teddsa_private_key", sk.clone())
        .build();
    let outputs = simulate("eddsa_sign", inputs)?;
    assert_eq!(outputs.len(), 1);
    let output = outputs.get("teddsa_signature").unwrap();
    if let NadaValue::EddsaSignature(signature) = output {
        let pk = ThresholdPublicKey::<Ed25519>::from_private_key(&sk);
        let verifies = verify(pk, signature.clone(), message);
        assert!(verifies);
        Ok(())
    } else {
        Err(anyhow!("Output should be a NadaValue::EddsaSignature"))
    }
}

// Since this is probabilistic, we're checking if the signature is valid
#[test]
fn eddsa_sign_with_public_key() -> Result<(), Error> {
    let sk = ThresholdPrivateKey::<Ed25519>::from_scalar(SecretScalar::random(&mut rand::thread_rng())).unwrap();
    let message = b"Some message to be signed";

    let inputs = StaticInputGeneratorBuilder::default()
        .add_eddsa_message("teddsa_message", message.to_vec())
        .add_eddsa_private_key("teddsa_private_key", sk.clone())
        .build();
    let outputs = simulate("eddsa_sign_with_public_key", inputs)?;
    assert_eq!(outputs.len(), 3);

    // Check EddsaPublicKey
    let out_public_key = if let NadaValue::EddsaPublicKey(pk) = outputs.get("teddsa_public_key").unwrap() {
        pk
    } else {
        panic!("Expected EddsaPublicKey")
    };
    let pk = ThresholdPublicKey::<Ed25519>::from_private_key(&sk);
    assert_eq!(pk.clone().to_bytes(true), out_public_key.to_vec());

    // Check EddsaMessage
    let out_message = if let NadaValue::EddsaMessage(msg) = outputs.get("teddsa_message").unwrap() {
        msg
    } else {
        panic!("Expected EddsaMessage")
    };
    assert_eq!(message.to_vec(), *out_message);

    // Check EddsaSignature
    let out_signature = if let NadaValue::EddsaSignature(sig) = outputs.get("teddsa_signature").unwrap() {
        sig
    } else {
        panic!("Expected EddsaSignature")
    };
    let verifies = verify(pk, out_signature.clone(), message);
    assert!(verifies);

    Ok(())
}
