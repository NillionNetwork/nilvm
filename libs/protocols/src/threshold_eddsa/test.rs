#![allow(clippy::arithmetic_side_effects, clippy::panic, clippy::indexing_slicing)]

use super::output::EddsaSignatureOutput;
use crate::{
    simulator::symmetric::{InitializedProtocol, Protocol, SymmetricProtocolSimulator},
    threshold_eddsa::state::EddsaSignState,
};
use anyhow::{anyhow, Error, Result};
use basic_types::PartyId;
use rstest::rstest;
use threshold_keypair::{
    privatekey::{ThresholdPrivateKey, ThresholdPrivateKeyShare},
    publickey::ThresholdPublicKey,
    signature::EddsaSignature,
};

use crate::threshold_ecdsa::util::SortedParties;
use cggmp21::generic_ec::{curves::Ed25519, NonZero, SecretScalar};
use givre::{signing::aggregate::Signature, Ciphersuite};

use rand_chacha::rand_core::OsRng;
use shamir_sharing::secret_sharer::PartyShares;

struct EddsaSignProtocol {
    private_key: ThresholdPrivateKey<Ed25519>,
    message: Vec<u8>,
}

impl EddsaSignProtocol {
    fn new(private_key: ThresholdPrivateKey<Ed25519>, message: Vec<u8>) -> Self {
        Self { private_key, message }
    }
}

impl Protocol for EddsaSignProtocol {
    type State = EddsaSignState;
    type PrepareOutput = EddsaSignConfig;

    fn prepare(&self, parties: &[PartyId]) -> Result<Self::PrepareOutput, Error> {
        let sorted_parties = SortedParties::new(parties.to_vec());
        let n: u16 = parties
            .len()
            .try_into()
            .map_err(|_| anyhow!("Failed to convert the length of parties (which is {}) to a u16", parties.len()))?;

        let pk_shares = self.private_key.generate_shares(n).map_err(|e| anyhow!("generation shares failed: {e}"))?;
        let mut private_key_shares: PartyShares<ThresholdPrivateKeyShare<Ed25519>> = PartyShares::default();
        for (party_id, pk_share) in sorted_parties.parties().iter().zip(pk_shares.iter()) {
            private_key_shares.insert(party_id.clone(), pk_share.clone());
        }

        Ok(EddsaSignConfig { parties: parties.to_vec(), private_key_shares, message: self.message.clone() })
    }

    fn initialize(
        &self,
        party_id: PartyId,
        config: &Self::PrepareOutput,
    ) -> Result<InitializedProtocol<Self::State>, anyhow::Error> {
        let key_share = config
            .private_key_shares
            .get(&party_id)
            .cloned()
            .ok_or_else(|| anyhow!("shares for party {party_id:?}"))?;

        let (state, messages) = EddsaSignState::new(config.parties.clone(), config.message.clone(), key_share)?;
        Ok(InitializedProtocol::new(state, messages))
    }
}

/// The internal configuration of a EcdsSignConfig protocol.
struct EddsaSignConfig {
    parties: Vec<PartyId>,
    private_key_shares: PartyShares<ThresholdPrivateKeyShare<Ed25519>>,
    message: Vec<u8>,
}

fn verify(pk: ThresholdPublicKey<Ed25519>, signature: EddsaSignature, message: Vec<u8>) -> bool {
    let givre_sig = Signature { r: signature.signature.r, z: signature.signature.z };
    let pk_point = NonZero::from_point(*pk.as_point()).expect("Public key should not be zero!");

    // Normalize the point using givre's normalize_point
    let pk_normalized = Ciphersuite::normalize_point(pk_point); // NormalizedPoint<_, Point<Ed25519>>

    // Call verify with the normalized and non-zero point
    givre_sig.verify(&pk_normalized, &message).is_ok()
}

#[rstest]
fn end_to_end() {
    //0. Network configuration
    let max_rounds = 100;
    let network_size = 3;
    // 1. Message generation
    let message_to_sign = b"Transaction with plenty of bitcoin".to_vec();
    // 2. Secret key generation
    let mut csprng = OsRng;
    let sk_val = SecretScalar::<Ed25519>::random(&mut csprng);
    let sk: ThresholdPrivateKey<Ed25519> = ThresholdPrivateKey::<Ed25519>::from_scalar(sk_val).unwrap();
    // 3. Run protocol
    let protocol = EddsaSignProtocol::new(sk.clone(), message_to_sign.clone());
    let simulator = SymmetricProtocolSimulator::new(network_size, max_rounds);
    let outputs = simulator.run_protocol(&protocol).expect("protocol run failed");
    // 4. Collect Signatures
    let mut signature = Vec::new();
    for output in outputs {
        match output.output {
            EddsaSignatureOutput::Success { element: sig } => {
                signature.push(sig);
            }
            EddsaSignatureOutput::Abort { reason } => {
                panic!("Aborted with reason: {:?}", reason);
            }
        }
    }
    // 5. Check signatures from all parties are the same
    if signature.iter().all(|sig| *sig != signature[0]) {
        panic!("Parties provided different signatures");
    }
    // 6. Verifies signature is valid under message_digest and public key
    let pk = ThresholdPublicKey::<Ed25519>::from_private_key(&sk);
    let verifies = verify(pk, signature[0].clone(), message_to_sign);
    assert!(verifies)
}
