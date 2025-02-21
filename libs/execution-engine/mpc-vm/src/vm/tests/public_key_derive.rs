use crate::vm::tests::simulate;
use anyhow::{anyhow, Error, Ok};
use cggmp21::generic_ec::{
    curves::{Ed25519, Secp256k1},
    SecretScalar,
};
use execution_engine_vm::simulator::inputs::StaticInputGeneratorBuilder;
use nada_value::NadaValue;
use threshold_keypair::{privatekey::ThresholdPrivateKey, publickey::ThresholdPublicKey};

#[test]
fn ecdsa_public_key_derive() -> Result<(), Error> {
    let sk = ThresholdPrivateKey::<Secp256k1>::from_scalar(SecretScalar::random(&mut rand::thread_rng())).unwrap();

    let inputs = StaticInputGeneratorBuilder::default().add_ecdsa_private_key("tecdsa_private_key", sk.clone()).build();
    let outputs = simulate("ecdsa_public_key_derive", inputs)?;
    assert_eq!(outputs.len(), 1);
    let output = outputs.get("tecdsa_public_key").unwrap();
    if let NadaValue::EcdsaPublicKey(public_key) = output {
        let pk_vec = ThresholdPublicKey::<Secp256k1>::from_private_key(&sk).to_bytes(true);
        let pk: [u8; 33] = pk_vec.try_into().unwrap();
        assert_eq!(public_key.0, pk);
        Ok(())
    } else {
        Err(anyhow!("Output should be a NadaValue::EcdsaPublicKey"))
    }
}

#[test]
fn eddsa_public_key_derive() -> Result<(), Error> {
    let sk = ThresholdPrivateKey::<Ed25519>::from_scalar(SecretScalar::random(&mut rand::thread_rng())).unwrap();

    let inputs = StaticInputGeneratorBuilder::default().add_eddsa_private_key("teddsa_private_key", sk.clone()).build();
    let outputs = simulate("eddsa_public_key_derive", inputs)?;
    assert_eq!(outputs.len(), 1);
    let output = outputs.get("teddsa_public_key").unwrap();
    if let NadaValue::EddsaPublicKey(public_key) = output {
        let pk_vec = ThresholdPublicKey::<Ed25519>::from_private_key(&sk).to_bytes(true);
        let pk: [u8; 32] = pk_vec.try_into().unwrap();
        assert_eq!(*public_key, pk);
        Ok(())
    } else {
        Err(anyhow!("Output should be a NadaValue::EddsaPublicKey"))
    }
}
