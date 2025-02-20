use crate::vm::tests::{secret_boolean, simulate};
use anyhow::{anyhow, Error, Ok};
use cggmp21::{
    generic_ec::{curves::Secp256k1, Scalar, SecretScalar},
    signing::{DataToSign, Signature},
};
use execution_engine_vm::simulator::inputs::StaticInputGeneratorBuilder;
use nada_value::NadaValue;
use threshold_keypair::{privatekey::ThresholdPrivateKey, publickey::ThresholdPublicKey, signature::EcdsaSignature};

fn verify(pk: ThresholdPublicKey<Secp256k1>, signature: EcdsaSignature, message: &DataToSign<Secp256k1>) -> bool {
    let EcdsaSignature { r, s } = signature;
    let cggmp_sig = Signature { r, s };

    let pk = pk.as_point();
    cggmp_sig.verify(pk, message).is_ok()
}

// Since this is probabilistic, we're checking if the signature is valid
#[test]
fn ecdsa_sign() -> Result<(), Error> {
    let sk = ThresholdPrivateKey::<Secp256k1>::from_scalar(SecretScalar::random(&mut rand::thread_rng())).unwrap();
    let digest = [
        76, 111, 114, 101, 109, 32, 105, 112, 115, 117, 109, 32, 100, 111, 108, 111, 114, 32, 115, 105, 116, 32, 97,
        109, 101, 116, 44, 32, 99, 111, 110, 115,
    ];

    let inputs = StaticInputGeneratorBuilder::default()
        .add_ecdsa_digest_message("tecdsa_digest_message", digest)
        .add_ecdsa_private_key("tecdsa_private_key", sk.clone())
        .build();
    let outputs = simulate("ecdsa_sign", inputs)?;
    assert_eq!(outputs.len(), 1);
    let output = outputs.get("tecdsa_signature").unwrap();
    if let NadaValue::EcdsaSignature(signature) = output {
        let pk = ThresholdPublicKey::<Secp256k1>::from_private_key(&sk);
        let digest_data_to_sign = DataToSign::from_scalar(Scalar::from_be_bytes_mod_order(digest));
        let verifies = verify(pk, signature.clone(), &digest_data_to_sign);
        assert!(verifies);
        Ok(())
    } else {
        Err(anyhow!("Output should be a NadaValue::EcdsaSignature"))
    }
}

// Since this is probabilistic, we're checking if the signature is valid
#[test]
fn ecdsa_sign_complex() -> Result<(), Error> {
    let sk = ThresholdPrivateKey::<Secp256k1>::from_scalar(SecretScalar::random(&mut rand::thread_rng())).unwrap();
    let digest = [
        76, 111, 114, 101, 109, 32, 105, 112, 115, 117, 109, 32, 100, 111, 108, 111, 114, 32, 115, 105, 116, 32, 97,
        109, 101, 116, 44, 32, 99, 111, 110, 115,
    ];

    let inputs = StaticInputGeneratorBuilder::default()
        .add_integer("public_my_int2", 2)
        .add_secret_integer("my_int1", 3)
        .add_ecdsa_digest_message("digest", digest)
        .add_ecdsa_private_key("private_key", sk.clone())
        .build();
    let outputs = simulate("ecdsa_sign_complex", inputs)?;
    assert_eq!(outputs.len(), 2);
    let output_1 = outputs.get("my_output").unwrap();
    let output_2 = outputs.get("my_output_result").unwrap();
    assert_eq!(output_2, &secret_boolean(true));

    if let NadaValue::EcdsaSignature(signature) = output_1 {
        let pk = ThresholdPublicKey::<Secp256k1>::from_private_key(&sk);
        let digest_data_to_sign = DataToSign::from_scalar(Scalar::from_be_bytes_mod_order(digest));
        let verifies = verify(pk, signature.clone(), &digest_data_to_sign);
        assert!(verifies);
        Ok(())
    } else {
        Err(anyhow!("Output should be a NadaValue::EcdsaSignature"))
    }
}
