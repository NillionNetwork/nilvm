use cggmp21::signing::{DataToSign, Signature};
use client_fixture::compute::{ClientsMode, ComputeValidator};
use generic_ec::{curves::Secp256k1, NonZero, Scalar, SecretScalar};
use k256::{
    ecdsa::{signature::Verifier, Signature as ecdsaSignature, SigningKey as ecdsaSigningKey},
    elliptic_curve::FieldBytes,
};
use nillion_client::{
    async_trait,
    grpc::{MembershipClient, TransportChannel},
    payments::{NilChainPayer, NillionChainClientPayer, TxHash},
    vm::PaymentMode,
    Clear, NadaType, NadaValue, SigningKey, TokenAmount,
};
use node_api::{
    compute::{
        proto::{compute_client::ComputeClient, stream::ComputeType},
        rust::ComputeStreamMessage,
        TECDSA_DKG_PROGRAM_ID, TECDSA_PUBLIC_KEY, TECDSA_SIGN_PROGRAM_ID, TECDSA_STORE_ID,
    },
    preprocessing::{
        proto::preprocessing_client::PreprocessingClient,
        rust::{GeneratePreprocessingRequest, PreprocessingElement, PreprocessingStreamMessage},
    },
    Code, ConvertProto,
};
use nodes_fixtures::{
    nodes::{nodes, Nodes},
    programs::PROGRAMS,
};
use rand_chacha::rand_core::OsRng;
use rstest::rstest;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use threshold_keypair::{privatekey::ThresholdPrivateKey, publickey::ThresholdPublicKey, signature::EcdsaSignature};
use tokio::sync::{mpsc::channel, Mutex};
use tokio_stream::wrappers::ReceiverStream;
use uuid::Uuid;

#[rstest]
#[tokio::test]
async fn cluster_definition(nodes: &Nodes) {
    let client = nodes.build_client().await;
    let expected_cluster = client.cluster();

    // ensure all members are part of the same cluster
    for member in &expected_cluster.members {
        let channel = nodes.node_channel_config(member.grpc_endpoint.clone()).build().expect("failed to build channel");
        let client = MembershipClient::new(channel);
        let cluster = client.cluster().await.expect("failed to get cluster");
        assert_eq!(&cluster, expected_cluster);
    }
}

#[rstest]
#[tokio::test]
async fn pool_status(nodes: &Nodes) {
    let client = nodes.build_client().await;
    client.pool_status().invoke().await.expect("failed to get pool status");
}

#[rstest]
#[tokio::test]
async fn node_version(nodes: &Nodes) {
    let client = nodes.build_client().await;
    let member = &client.cluster().leader;
    client.node_version(member.identity.clone()).await.expect("failed to get node version");
}

#[rstest]
#[tokio::test]
async fn store_program(nodes: &Nodes) {
    let client = nodes.build_client().await;
    let program = PROGRAMS.metadata("reduce_simple").expect("'simple' program not found").raw_mir();
    client
        .store_program()
        .name("my-program")
        .program(program)
        .build()
        .expect("failed to build operation")
        .invoke()
        .await
        .expect("failed to store program");
}

#[rstest]
#[tokio::test]
async fn store_invalid_program_name(nodes: &Nodes) {
    let client = nodes.build_client().await;
    let program = PROGRAMS.metadata("reduce_simple").expect("'simple' program not found").raw_mir();
    client
        .store_program()
        .name("my-program/hi")
        .program(program)
        .build()
        .map(|_| ())
        .expect_err("failed to build operation");
}

#[rstest]
#[tokio::test]
async fn store_invalid_program(nodes: &Nodes) {
    let client = nodes.build_client().await;
    let program = test_programs::PROGRAMS.metadata("invalid_program").expect("'invalid_program' not found").raw_mir();
    client
        .store_program()
        .name("invalid-program")
        .program(program)
        .build()
        .expect("failed to build operation")
        .invoke()
        .await
        .expect_err("upload didn't fail");
}

#[rstest]
#[tokio::test]
async fn store_values(nodes: &Nodes) {
    let client = nodes.build_client().await;
    let values_id = client
        .store_values()
        .add_value("foo", NadaValue::new_secret_integer(42))
        .add_value("bar", NadaValue::new_integer(1337))
        .ttl_days(3)
        .build()
        .expect("failed to build operation")
        .invoke()
        .await
        .expect("failed to store values");
    assert_eq!(values_id.get_version(), Some(uuid::Version::Random));
}

#[rstest]
#[tokio::test]
async fn update_values(nodes: &Nodes) {
    // Create ecdsa private key Nada value
    let mut csprng = OsRng;
    let sk = SecretScalar::<Secp256k1>::random(&mut csprng);
    let ecdsa_sk = ThresholdPrivateKey::from_scalar(sk).unwrap();
    let sk_update = SecretScalar::<Secp256k1>::random(&mut csprng);
    let ecdsa_sk_update = ThresholdPrivateKey::from_scalar(sk_update).unwrap();

    let client = nodes.build_client().await;
    let store_identifier = client
        .store_values()
        .add_value("foo", NadaValue::new_secret_integer(42))
        .add_value("foo_ecdsa", NadaValue::new_ecdsa_private_key(ecdsa_sk))
        .ttl_days(3)
        .build()
        .expect("failed to build operation")
        .invoke()
        .await
        .expect("failed to store values");
    let update_identifier = client
        .store_values()
        .add_value("bar", NadaValue::new_secret_integer(1337))
        .add_value("bar_ecdsa", NadaValue::new_ecdsa_private_key(ecdsa_sk_update))
        .ttl_days(3)
        .update_identifier(store_identifier)
        .build()
        .expect("failed to build operation")
        .invoke()
        .await
        .expect("failed to update values");
    assert_eq!(store_identifier, update_identifier);

    let values = client
        .retrieve_values()
        .values_id(update_identifier)
        .build()
        .expect("failed to build")
        .invoke()
        .await
        .expect("failed to retrieve values");
    assert!(values.contains_key("bar"), "'bar' missing: {values:?}");
    assert!(values.contains_key("bar_ecdsa"), "'bar_ecdsa' missing: {values:?}");
}

#[rstest]
#[tokio::test]
async fn delete_values(nodes: &Nodes) {
    let client = nodes.build_client().await;
    let values_id = client
        .store_values()
        .add_value("foo", NadaValue::new_secret_integer(42))
        .ttl_days(3)
        .build()
        .expect("failed to build operation")
        .invoke()
        .await
        .expect("failed to store values");
    client
        .delete_values()
        .values_id(values_id)
        .build()
        .expect("build failed")
        .invoke()
        .await
        .expect("failed to delete");
}

#[rstest]
#[tokio::test]
async fn retrieve_values(nodes: &Nodes) {
    // Create ecdsa private key Nada value
    let mut csprng = OsRng;
    let sk = SecretScalar::<Secp256k1>::random(&mut csprng);
    let ecdsa_sk = ThresholdPrivateKey::from_scalar(sk).unwrap();

    let expected_values: HashMap<String, NadaValue<Clear>> = [
        ("a".into(), NadaValue::new_secret_integer(42)),
        ("b".into(), NadaValue::new_integer(42)),
        ("c".into(), NadaValue::new_secret_boolean(false)),
        ("d".into(), NadaValue::new_secret_blob(vec![1, 2, 3])),
        ("g".into(), NadaValue::new_ecdsa_private_key(ecdsa_sk)),
    ]
    .into();
    let client = nodes.build_client().await;
    let values_id = client
        .store_values()
        .add_values(expected_values.clone().into_iter())
        .ttl_days(3)
        .build()
        .expect("failed to build operation")
        .invoke()
        .await
        .expect("failed to store values");

    let values = client
        .retrieve_values()
        .values_id(values_id)
        .build()
        .expect("failed to build")
        .invoke()
        .await
        .expect("failed to retrieve values");
    assert_eq!(values, expected_values);
}

#[rstest]
#[tokio::test]
async fn store_retrieve_update_ecdsa_private_keys_digest_msg_and_signature(nodes: &Nodes) {
    // Create ecdsa private key
    let mut csprng = OsRng;
    let sk = SecretScalar::<Secp256k1>::random(&mut csprng);
    let ecdsa_sk = ThresholdPrivateKey::from_scalar(sk).unwrap();
    let sk_update = SecretScalar::<Secp256k1>::random(&mut csprng);
    let ecdsa_sk_update = ThresholdPrivateKey::from_scalar(sk_update).unwrap();
    // Create ecdsa digest message
    let msg_digest: [u8; 32] =
        [0, 11, 1, 49, 1, 8, 42, 0, 0, 0, 0, 0, 0, 0, 0, 2, 49, 48, 15, 11, 8, 11, 3, 2, 5, 56, 18, 39, 20, 0, 21, 42];
    let msg_digest_update: [u8; 32] =
        [0, 11, 1, 49, 1, 8, 42, 0, 0, 1, 1, 1, 0, 0, 0, 2, 49, 48, 15, 11, 8, 11, 3, 2, 5, 56, 18, 39, 20, 0, 21, 42];
    // Create ecdsa signature
    let r = NonZero::from_scalar(Scalar::<Secp256k1>::random(&mut csprng)).unwrap();
    let s = NonZero::from_scalar(Scalar::<Secp256k1>::random(&mut csprng)).unwrap();
    let ecdsa_sig = EcdsaSignature { r, s }.normalize_s();
    let r = NonZero::from_scalar(Scalar::<Secp256k1>::random(&mut csprng)).unwrap();
    let s = NonZero::from_scalar(Scalar::<Secp256k1>::random(&mut csprng)).unwrap();
    let ecdsa_sig_update = EcdsaSignature { r, s }.normalize_s();

    let expected_values: HashMap<String, NadaValue<Clear>> = [
        ("ecdsa".into(), NadaValue::new_ecdsa_private_key(ecdsa_sk)),
        ("msg_digest".into(), NadaValue::new_ecdsa_digest_message(msg_digest)),
        ("signature".into(), NadaValue::new_ecdsa_signature(ecdsa_sig)),
    ]
    .into();
    let expected_updated_values: HashMap<String, NadaValue<Clear>> = [
        ("ecdsa_update".into(), NadaValue::new_ecdsa_private_key(ecdsa_sk_update)),
        ("msg_digest_update".into(), NadaValue::new_ecdsa_digest_message(msg_digest_update)),
        ("signature_update".into(), NadaValue::new_ecdsa_signature(ecdsa_sig_update)),
    ]
    .into();

    // Store
    let client = nodes.build_client().await;
    let store_identifier = client
        .store_values()
        .add_values(expected_values.clone().into_iter())
        .ttl_days(3)
        .build()
        .expect("failed to build operation")
        .invoke()
        .await
        .expect("failed to store values");
    // Retrieve
    let values = client
        .retrieve_values()
        .values_id(store_identifier)
        .build()
        .expect("failed to build")
        .invoke()
        .await
        .expect("failed to retrieve values");
    // Compare stored and retrieved
    assert_eq!(values, expected_values);
    // Update
    let update_identifier = client
        .store_values()
        .add_values(expected_updated_values.clone().into_iter())
        .ttl_days(3)
        .update_identifier(store_identifier)
        .build()
        .expect("failed to build operation")
        .invoke()
        .await
        .expect("failed to update values");
    assert_eq!(store_identifier, update_identifier);
    // Retrieve updated
    let updated_values = client
        .retrieve_values()
        .values_id(update_identifier)
        .build()
        .expect("failed to build")
        .invoke()
        .await
        .expect("failed to retrieve values");
    assert!(updated_values.contains_key("ecdsa_update"), "'ecdsa_update' missing: {updated_values:?}");
    // Compare stored and retrieved
    assert_eq!(updated_values, expected_updated_values);
}

#[rstest]
#[tokio::test]
async fn values_permission_denied(nodes: &Nodes) {
    let client = nodes.build_client().await;
    let values_id = client
        .store_values()
        .add_value("a", NadaValue::new_secret_integer(42))
        .ttl_days(3)
        .build()
        .expect("failed to build operation")
        .invoke()
        .await
        .expect("failed to store values");

    let other_client = nodes.build_client().await;
    other_client
        .retrieve_values()
        .values_id(values_id)
        .build()
        .expect("failed to build")
        .invoke()
        .await
        .expect_err("retrieving values succeeded");
    other_client
        .store_values()
        .add_value("b", NadaValue::new_integer(1337))
        .ttl_days(3)
        .update_identifier(values_id)
        .build()
        .expect("failed to build")
        .invoke()
        .await
        .expect_err("updating values succeeded");
}

#[rstest]
#[tokio::test]
async fn retrieve_unknown_value_id(nodes: &Nodes) {
    let client = nodes.build_client().await;
    client
        .retrieve_values()
        .values_id(Uuid::new_v4())
        .build()
        .expect("failed to build")
        .invoke()
        .await
        .expect_err("retrieving values succeeded");
}

#[rstest]
#[tokio::test]
async fn retrieve_permissions(nodes: &Nodes) {
    let client = nodes.build_client().await;
    let values_id = client
        .store_values()
        .add_value("foo", NadaValue::new_secret_integer(42))
        .ttl_days(3)
        .build()
        .expect("failed to build operation")
        .invoke()
        .await
        .expect("failed to store values");

    let user_id = client.user_id();
    let permissions = client
        .retrieve_permissions()
        .values_id(values_id)
        .build()
        .expect("failed to build")
        .invoke()
        .await
        .expect("failed to retrieve permissions");

    assert_eq!(permissions.owner, user_id);
    assert!(permissions.retrieve.contains(&user_id));
    assert!(permissions.update.contains(&user_id));
    assert!(permissions.delete.contains(&user_id));
}

#[rstest]
#[tokio::test]
async fn update_permissions(nodes: &Nodes) {
    let client = nodes.build_client().await;
    let user_id = client.user_id();

    let values_id = client
        .store_values()
        .add_value("bar", NadaValue::new_secret_integer(109))
        .ttl_days(3)
        .build()
        .expect("failed to build operation")
        .invoke()
        .await
        .expect("failed to store value");

    let permissions = client
        .retrieve_permissions()
        .values_id(values_id)
        .build()
        .expect("failed to build")
        .invoke()
        .await
        .expect("failed to retrieve permissions");

    assert_eq!(permissions.owner, user_id);

    // Create 2nd client
    let client2 = nodes.build_client().await;
    let user_id_2 = client2.user_id();

    // Add retrieve, update & compute permissions to client2 with update_permissions builder
    let program_id = nodes.uploaded_programs.program_id("simple_shares");
    client
        .overwrite_permissions()
        .values_id(values_id)
        .allow_retrieve(user_id_2.clone())
        .allow_update(user_id_2.clone())
        .allow_compute(user_id_2.clone(), program_id.clone())
        .build()
        .expect("falied to build update_permission operation")
        .invoke()
        .await
        .expect("failed to update permissions");

    // Fetch updated permissions
    let updated_permissions = client
        .retrieve_permissions()
        .values_id(values_id)
        .build()
        .expect("failed to build")
        .invoke()
        .await
        .expect("failed to retrieve permissions");

    // Verify new permissions for client2
    assert!(updated_permissions.retrieve.contains(&user_id_2));
    assert!(updated_permissions.update.contains(&user_id_2));
    assert!(!updated_permissions.delete.contains(&user_id_2));

    match updated_permissions.compute.get(&user_id_2) {
        Some(permissions) => assert!(permissions.program_ids.contains(&program_id)),
        None => panic!("allow compute permissions test failed"),
    }
}

#[rstest]
#[tokio::test]
async fn manage_permissions(nodes: &Nodes) {
    let client = nodes.build_client().await;
    let other_client = nodes.build_client().await;
    let values_id = client
        .store_values()
        .add_value("foo", NadaValue::new_secret_integer(42))
        .ttl_days(3)
        .build()
        .expect("failed to build operation")
        .invoke()
        .await
        .expect("failed to store values");

    // grant retrieve and retrieve with new client
    let user = other_client.user_id().clone();
    client
        .update_permissions()
        .values_id(values_id)
        .grant_retrieve(user)
        .build()
        .unwrap()
        .invoke()
        .await
        .expect("failed to invoke");
    other_client.retrieve_values().values_id(values_id).build().unwrap().invoke().await.expect("failed to fetch");

    // revoke retrieve and try to retrieve with new client
    let user = other_client.user_id().clone();
    client
        .update_permissions()
        .values_id(values_id)
        .revoke_retrieve(user)
        .build()
        .unwrap()
        .invoke()
        .await
        .expect("failed to invoke");
    other_client
        .retrieve_values()
        .values_id(values_id)
        .build()
        .unwrap()
        .invoke()
        .await
        .expect_err("fetching succeeded");

    // original client should always retain permissions
    client.retrieve_values().values_id(values_id).build().unwrap().invoke().await.expect("failed to fetch");
}

#[rstest]
#[case::simple_shares("simple_shares")]
#[case::simple_public_variables("simple_public_variables")]
#[case::simple_subtraction("simple_subtraction")]
#[case::simple_subtraction_negative("simple_subtraction_negative")]
#[case::simple_subtraction_public_variables("simple_subtraction_public-variables")]
#[case::multi_output("multi_output")]
#[case::array_simple_shares("array_simple_shares")]
#[case::array_new("array_new")]
#[case::less_than("less_than")]
#[case::three_dealer_product("3dealer-product")]
#[case::greater_or_equal_than("greater_or_equal_than")]
#[tokio::test]
async fn invoke_compute(nodes: &Nodes, #[case] program_name: &str) {
    let (program, bytecode) = PROGRAMS.program(program_name).expect("program not found");
    let program_id = nodes.uploaded_programs.program_id(program_name);
    ComputeValidator::builder().program_id(program_id).program(program, bytecode).run(nodes).await;
}

fn verify(pk: ThresholdPublicKey<Secp256k1>, signature: EcdsaSignature, message: &DataToSign<Secp256k1>) -> bool {
    let EcdsaSignature { r, s } = signature;
    let cggmp_sig = Signature { r, s };

    let pk = pk.as_point();
    cggmp_sig.verify(pk, message).is_ok()
}

#[rstest]
#[tokio::test]
async fn tecdsa_sign(nodes: &Nodes) {
    let program_id = TECDSA_SIGN_PROGRAM_ID;

    let message = b"This is my message that is going be get signed";
    let digest: [u8; 32] = Sha256::digest(message).try_into().expect("digest generation failure");

    // external library keys
    let external_sk = ecdsaSigningKey::random(&mut rand::thread_rng());
    let external_pk = external_sk.verifying_key();

    let sk_bytes: &[u8] = &external_sk.to_bytes();

    // cggmp21 library keys
    let cggmp21_sk = ThresholdPrivateKey::from_be_bytes(&sk_bytes).expect("ecdsa private from bytes have failed");
    let cggmp21_pk = ThresholdPublicKey::from_private_key(&cggmp21_sk);

    let client = nodes.build_client().await;
    let compute_id = client
        .invoke_compute()
        .program_id(program_id)
        .add_value("tecdsa_private_key", NadaValue::new_ecdsa_private_key(cggmp21_sk.clone()))
        .add_value("tecdsa_digest_message", NadaValue::new_ecdsa_digest_message(digest))
        .bind_input_party("tecdsa_key_party", client.user_id())
        .bind_input_party("tecdsa_digest_message_party", client.user_id())
        .bind_output_party("tecdsa_output_party", [client.user_id()])
        .build()
        .expect("build failure")
        .invoke()
        .await
        .expect("fetching succeeded");

    let outputs = client
        .retrieve_compute_results()
        .compute_id(compute_id)
        .build()
        .expect("failed to build")
        .invoke()
        .await
        .expect("failed to get the result")
        .expect("error");

    let output = outputs.get("tecdsa_signature").unwrap();

    if let NadaValue::EcdsaSignature(cggmp21_signature) = output {
        // verify with cggmp21 library
        let digest_data_to_sign = DataToSign::from_scalar(Scalar::from_be_bytes_mod_order(digest));
        let verifies = verify(cggmp21_pk, cggmp21_signature.clone(), &digest_data_to_sign);
        assert!(verifies);

        // Transform cggmp21 signature into external signature
        let EcdsaSignature { r, s } = cggmp21_signature;
        let r_bytes = r.to_be_bytes();
        let r_bytes = r_bytes.as_bytes();
        let s_bytes = s.to_be_bytes();
        let s_bytes = s_bytes.as_bytes();
        let r = FieldBytes::<k256::Secp256k1>::clone_from_slice(r_bytes);
        let s = FieldBytes::<k256::Secp256k1>::clone_from_slice(s_bytes);
        let to_external_signature =
            ecdsaSignature::from_scalars(r, s).expect("signature generation from scalars failed");

        // verify with external library
        let external_verifies = external_pk.verify(message, &to_external_signature).is_ok();
        assert!(external_verifies);
    } else {
        panic!("Output should be a NadaValue::EcdsaSignature");
    }
}

#[rstest]
#[tokio::test]
async fn dkg_no_parties_bound(nodes: &Nodes) {
    let client = nodes.build_client().await;
    client
        .invoke_compute()
        .program_id(TECDSA_DKG_PROGRAM_ID)
        .build()
        .expect("build failure")
        .invoke()
        .await
        .expect_err("invoke succeeded");
}

#[rstest]
#[tokio::test]
async fn dkg_and_retrieve_private_key(nodes: &Nodes) {
    // Tests:
    // 1. Generate the public key and private key
    // 2. Retrieve the created private key
    // 3. Check public key provided by compute matches public key derived from private key
    let program_id = TECDSA_DKG_PROGRAM_ID;

    // Generate the public key and private key
    let client = nodes.build_client().await;
    let compute_id = client
        .invoke_compute()
        .program_id(program_id)
        .bind_output_party("tecdsa_private_key_store_id_party", [client.user_id()])
        .bind_output_party("tecdsa_public_key_party", [client.user_id()])
        .build()
        .expect("build failure")
        .invoke()
        .await
        .expect("fetching succeeded");

    let outputs = client
        .retrieve_compute_results()
        .compute_id(compute_id)
        .build()
        .expect("failed to build")
        .invoke()
        .await
        .expect("failed to get the result")
        .expect("error");

    // Get the store id and verify it's a NadaValue::StoreId
    let store_id = outputs.get(TECDSA_STORE_ID).unwrap();
    let store_id = if let NadaValue::StoreId(store_id) = store_id {
        let uuid = Uuid::from_bytes(*store_id);
        uuid
    } else {
        panic!("Output should be a NadaValue::StoreId");
    };
    // Get the public key and verify it's a NadaValue::EcdsaPublicKey
    let tecdsa_public_key = outputs.get(TECDSA_PUBLIC_KEY).unwrap();
    let public_key = if let NadaValue::EcdsaPublicKey(public_key) = tecdsa_public_key {
        public_key.0
    } else {
        panic!("Output should be a NadaValue::EcdsaPublicKey");
    };

    // Retrieve the created private key
    let values = client
        .retrieve_values()
        .values_id(store_id)
        .build()
        .expect("failed to build")
        .invoke()
        .await
        .expect("failed to get store");

    // Verify private key is a NadaValue::EcdsaPrivateKey
    let private_key = values.get("tecdsa_private_key").unwrap();
    let private_key = if let NadaValue::EcdsaPrivateKey(private_key) = private_key {
        private_key
    } else {
        panic!("Store value should be a NadaValue::EcdsaPrivateKey");
    };

    // Check public key provided by compute matches public key derived from private key
    let external_sk = k256::SecretKey::from_bytes(
        private_key.clone().to_be_bytes().as_slice().try_into().expect("Invalid private key length"),
    )
    .expect("failed to convert private key to external type");
    let external_pk = external_sk.public_key();
    let computed_pk =
        k256::PublicKey::from_sec1_bytes(&public_key).expect("failed to convert public key to external type");
    assert_eq!(external_pk, computed_pk, "Public key does not match private key");
}

#[rstest]
#[tokio::test]
async fn dkg_and_sign(nodes: &Nodes) {
    // Tests:
    // 1. Generate the public key and private key
    // 2. Sign a message and verify the signature with the public key
    let dkg_program_id = TECDSA_DKG_PROGRAM_ID;
    let sign_program_id = TECDSA_SIGN_PROGRAM_ID;

    // Generate the public key and private key
    let client = nodes.build_client().await;
    let compute_id = client
        .invoke_compute()
        .program_id(dkg_program_id)
        .bind_output_party("tecdsa_private_key_store_id_party", [client.user_id()])
        .bind_output_party("tecdsa_public_key_party", [client.user_id()])
        .build()
        .expect("build failure")
        .invoke()
        .await
        .expect("fetching succeeded");

    let outputs = client
        .retrieve_compute_results()
        .compute_id(compute_id)
        .build()
        .expect("failed to build")
        .invoke()
        .await
        .expect("failed to get the result")
        .expect("error");

    // Get the store id and verify it's a NadaValue::StoreId
    let store_id = outputs.get(TECDSA_STORE_ID).unwrap();
    let store_id = if let NadaValue::StoreId(store_id) = store_id {
        let uuid = Uuid::from_bytes(*store_id);
        uuid
    } else {
        panic!("Output should be a NadaValue::StoreId");
    };
    // Get the public key and verify it's a NadaValue::EcdsaPublicKey
    let tecdsa_public_key = outputs.get(TECDSA_PUBLIC_KEY).unwrap();
    let public_key = if let NadaValue::EcdsaPublicKey(public_key) = tecdsa_public_key {
        public_key.0
    } else {
        panic!("Output should be a NadaValue::EcdsaPublicKey");
    };

    let message = b"This is my message that is going be get signed";
    let digest: [u8; 32] = Sha256::digest(message).try_into().expect("digest generation failure");

    // external library keys
    let external_pk = k256::ecdsa::VerifyingKey::from_sec1_bytes(&public_key).expect("failed to convert public key");

    // cggmp21 library keys
    let cggmp21_pk = ThresholdPublicKey::from_bytes(&public_key).expect("failed to convert public key to cggmp21 type");

    let compute_id = client
        .invoke_compute()
        .program_id(sign_program_id)
        .add_value_id(store_id)
        .add_value("tecdsa_digest_message", NadaValue::new_ecdsa_digest_message(digest))
        .bind_input_party("tecdsa_key_party", client.user_id())
        .bind_input_party("tecdsa_digest_message_party", client.user_id())
        .bind_output_party("tecdsa_output_party", [client.user_id()])
        .build()
        .expect("build failure")
        .invoke()
        .await
        .expect("fetching succeeded");

    let outputs = client
        .retrieve_compute_results()
        .compute_id(compute_id)
        .build()
        .expect("failed to build")
        .invoke()
        .await
        .expect("failed to get the result")
        .expect("error");

    let output = outputs.get("tecdsa_signature").unwrap();

    if let NadaValue::EcdsaSignature(cggmp21_signature) = output {
        // verify with cggmp21 library
        let digest_data_to_sign = DataToSign::from_scalar(Scalar::from_be_bytes_mod_order(digest));
        let verifies = verify(cggmp21_pk, cggmp21_signature.clone(), &digest_data_to_sign);
        assert!(verifies);

        // Transform cggmp21 signature into external signature
        let EcdsaSignature { r, s } = cggmp21_signature;
        let r_bytes = r.to_be_bytes();
        let r_bytes = r_bytes.as_bytes();
        let s_bytes = s.to_be_bytes();
        let s_bytes = s_bytes.as_bytes();
        let r = FieldBytes::<k256::Secp256k1>::clone_from_slice(r_bytes);
        let s = FieldBytes::<k256::Secp256k1>::clone_from_slice(s_bytes);
        let to_external_signature =
            ecdsaSignature::from_scalars(r, s).expect("signature generation from scalars failed");

        // verify with external library
        let external_verifies = external_pk.verify(message, &to_external_signature).is_ok();
        assert!(external_verifies);
    } else {
        panic!("Output should be a NadaValue::EcdsaSignature");
    }
}

#[rstest]
#[case::single_client(ClientsMode::Single)]
#[case::multiple_clients(ClientsMode::OnePerParty)]
#[tokio::test]
async fn multi_dealer_and_result(nodes: &Nodes, #[case] mode: ClientsMode) {
    let program_name = "multi-dealer-and-result";
    let program_id = nodes.uploaded_programs.program_id(program_name);
    let (program, bytecode) = PROGRAMS.program(program_name).expect("program not found");
    ComputeValidator::builder().program_id(program_id).program(program, bytecode).clients_mode(mode).run(nodes).await;
}

#[rstest]
#[tokio::test]
async fn pay_with_funds(nodes: &Nodes) {
    let payer = nodes.allocate_payer().await;
    // Generate a custom client with a random key to make sure it's not pre-funded
    let client = nodes
        .build_custom_client(|builder| builder.nilchain_payer(payer).signing_key(SigningKey::generate_secp256k1()))
        .await;
    let balance = client.account_balance().await.expect("failed to look up").balance;
    assert_eq!(balance, 0);

    // 1 nil = 100 credits at a $1 rate
    let added_nils = 1;
    let added_credits = 100;
    client
        .add_funds()
        .amount(TokenAmount::Nil(added_nils))
        .build()
        .expect("build failed")
        .invoke()
        .await
        .expect("add funds failed");
    let balance = client.account_balance().await.expect("failed to look up").balance;
    assert_eq!(balance, added_credits);

    client
        .store_values()
        .add_value("foo", NadaValue::new_secret_integer(42))
        .build()
        .expect("failed to build")
        .invoke()
        .await
        .expect("failed to invoke");
    let balance = client.account_balance().await.expect("failed to look up").balance;
    assert!(balance < added_credits, "{balance} >= {added_credits}");
}

#[rstest]
#[tokio::test]
async fn add_too_few_funds(nodes: &Nodes) {
    let client = nodes.build_client().await;

    client
        .add_funds()
        .amount(TokenAmount::Unil(1))
        .build()
        .expect("build failed")
        .invoke()
        .await
        .expect_err("add funds succeeded");
}

#[rstest]
#[tokio::test]
async fn compute_unauthorized(nodes: &Nodes) {
    let client = nodes.build_client().await;
    let user_id = client.user_id().clone();
    let program_id = nodes.uploaded_programs.program_id("array_simple_shares");
    let values_id = client
        .store_values()
        .add_value(
            "I00",
            NadaValue::new_array(NadaType::SecretInteger, vec![NadaValue::new_secret_integer(42)]).unwrap(),
        )
        .ttl_days(3)
        .build()
        .expect("failed to build operation")
        .invoke()
        .await
        .expect("failed to store values");
    let invoker_client = nodes.build_client().await;
    invoker_client
        .invoke_compute()
        .program_id(program_id)
        .add_value_id(values_id)
        .bind_input_party("Party1", user_id.clone())
        .bind_output_party("Party1", vec![user_id])
        .build()
        .unwrap()
        .invoke()
        .await
        .expect_err("not a failure");
}

#[rstest]
#[tokio::test]
async fn invoke_stream_preprocessing(nodes: &Nodes) {
    let grpc_channel = nodes.bootnode_channel(SigningKey::generate_secp256k1());
    let mut client = PreprocessingClient::new(grpc_channel.into_channel());
    let (tx, rx) = channel(16);
    tx.send(
        PreprocessingStreamMessage {
            generation_id: Uuid::new_v4().as_bytes().to_vec(),
            element: PreprocessingElement::Compare,
            bincode_message: vec![],
        }
        .into_proto(),
    )
    .await
    .unwrap();
    let result = client.stream_preprocessing(ReceiverStream::new(rx)).await.expect_err("request succeeded");
    assert_eq!(result.code(), Code::PermissionDenied);
}

#[rstest]
#[tokio::test]
async fn invoke_generate_preprocessing(nodes: &Nodes) {
    let grpc_channel = nodes.bootnode_channel(SigningKey::generate_secp256k1());
    let mut client = PreprocessingClient::new(grpc_channel.into_channel());
    let request = GeneratePreprocessingRequest {
        generation_id: Uuid::new_v4().as_bytes().to_vec(),
        batch_id: 0,
        batch_size: 16,
        element: PreprocessingElement::Compare,
    }
    .into_proto();
    let result = client.generate_preprocessing(request).await.expect_err("request succeeded");
    assert_eq!(result.code(), Code::PermissionDenied);
}

#[rstest]
#[tokio::test]
async fn invoke_stream_compute(nodes: &Nodes) {
    let grpc_channel = nodes.bootnode_channel(SigningKey::generate_secp256k1());
    let mut client = ComputeClient::new(grpc_channel.into_channel());
    let (tx, rx) = channel(16);
    tx.send(
        ComputeStreamMessage {
            compute_id: Uuid::new_v4().as_bytes().to_vec(),
            bincode_message: vec![],
            compute_type: ComputeType::General.into(),
        }
        .into_proto(),
    )
    .await
    .unwrap();
    let result = client.stream_compute(ReceiverStream::new(rx)).await.expect_err("request succeeded");
    assert_eq!(result.code(), Code::PermissionDenied);
}

#[rstest]
#[tokio::test]
async fn reuse_payment(nodes: &Nodes) {
    struct Payer {
        payer: NillionChainClientPayer,
        nonce: Mutex<Option<Vec<u8>>>,
    }
    #[async_trait]
    impl NilChainPayer for Payer {
        async fn submit_payment(
            &self,
            amount_unil: u64,
            resource: Vec<u8>,
        ) -> Result<TxHash, Box<dyn std::error::Error>> {
            // if we have a nonce use it, otherwise use `resource`
            let nonce = self.nonce.lock().await.clone().unwrap_or(resource.clone());
            let tx_hash = self.payer.submit_payment(amount_unil, nonce).await?;
            // save it for the next run
            *self.nonce.lock().await = Some(resource);
            Ok(tx_hash)
        }
    }
    let payer = Payer { payer: nodes.allocate_payer().await, nonce: Default::default() };
    let client = nodes
        .build_custom_client(|builder| builder.nilchain_payer(payer).payment_mode(PaymentMode::PayPerOperation))
        .await;
    client
        .store_values()
        .ttl_days(1)
        .add_value("foo", NadaValue::new_secret_integer(1))
        .build()
        .expect("failed to build")
        .invoke()
        .await
        .expect("failed to invoke");

    // This second time we'll reuse a nonce we used in the first run
    client
        .store_values()
        .ttl_days(1)
        .add_value("foo", NadaValue::new_secret_integer(1))
        .build()
        .expect("failed to build")
        .invoke()
        .await
        .expect_err("invocation succeeded");
}
