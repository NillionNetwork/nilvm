#![allow(clippy::arithmetic_side_effects, clippy::panic, clippy::indexing_slicing)]

use super::output::EcdsaSignatureShareOutput;
use crate::{
    simulator::symmetric::{InitializedProtocol, Protocol, SymmetricProtocolSimulator},
    threshold_ecdsa::{
        auxiliary_information::output::{EcdsaAuxInfo, EcdsaAuxInfoOutput},
        signing::EcdsaSignState,
    },
};
use anyhow::{anyhow, Error, Result};
use basic_types::PartyId;
use ecdsa_keypair::{privatekey::EcdsaPrivateKeyShare, signature::EcdsaSignatureShare};
use rstest::rstest;

use crate::threshold_ecdsa::{auxiliary_information::fake::FakeEcdsaAuxInfo, util::SortedParties};
use cggmp21::{
    generic_ec::{curves::Secp256k1, SecretScalar},
    signing::{DataToSign, Signature},
};
use ecdsa_keypair::{privatekey::EcdsaPrivateKey, publickey::EcdsaPublicKey, signature::EcdsaSignature};
use rand_chacha::rand_core::OsRng;
use sha2::Sha256;
use shamir_sharing::secret_sharer::PartyShares;

struct EcdsaSignProtocol {
    eid: Vec<u8>,
    private_key: EcdsaPrivateKey,
    message_digest: [u8; 32],
}

impl EcdsaSignProtocol {
    fn new(eid: Vec<u8>, private_key: EcdsaPrivateKey, message_digest: [u8; 32]) -> Self {
        Self { eid, private_key, message_digest }
    }

    fn create_aux_info(&self, parties: &[PartyId]) -> Result<PartyShares<EcdsaAuxInfo>, Error> {
        let parties = SortedParties::new(parties.to_vec());
        let n: u16 = parties
            .len()
            .try_into()
            .map_err(|_| anyhow!("Failed to convert the length of parties (which is {}) to a u16", parties.len()))?;

        let aux_info_output =
            match FakeEcdsaAuxInfo::generate_ecdsa(n).map_err(|_| anyhow!("Failed to generate aux_info"))? {
                EcdsaAuxInfoOutput::Success { element } => element,
                _ => return Err(anyhow!("Unexpected variant for EcdsaAuxInfoOutput")),
            };

        // Build PartyShares
        let mut shares: PartyShares<EcdsaAuxInfo> = PartyShares::default();
        for party_id in parties.parties() {
            shares.insert(party_id.clone(), aux_info_output.clone());
        }

        Ok(shares)
    }
}

impl Protocol for EcdsaSignProtocol {
    type State = EcdsaSignState;
    type PrepareOutput = EcdsSignConfig;

    fn prepare(&self, parties: &[PartyId]) -> Result<Self::PrepareOutput, Error> {
        let sorted_parties = SortedParties::new(parties.to_vec());
        let n: u16 = parties
            .len()
            .try_into()
            .map_err(|_| anyhow!("Failed to convert the length of parties (which is {}) to a u16", parties.len()))?;

        let aux_info = self.create_aux_info(&parties).map_err(|e| anyhow!("Aux info creation failed: {e}"))?;

        let pk_shares = self.private_key.generate_shares(n).map_err(|e| anyhow!("generation shares failed: {e}"))?;
        let mut private_key_shares: PartyShares<EcdsaPrivateKeyShare> = PartyShares::default();
        for (party_id, pk_share) in sorted_parties.parties().iter().zip(pk_shares.iter()) {
            private_key_shares.insert(party_id.clone(), pk_share.clone());
        }

        Ok(EcdsSignConfig {
            eid: self.eid.clone(),
            parties: parties.to_vec(),
            private_key_shares,
            aux_info,
            message_digest: self.message_digest,
        })
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
        let aux_info =
            config.aux_info.get(&party_id).cloned().ok_or_else(|| anyhow!("shares for party {party_id:?}"))?;

        let (state, messages) = EcdsaSignState::new(
            config.eid.clone(),
            config.parties.clone(),
            party_id,
            key_share,
            aux_info,
            config.message_digest,
        )?;

        Ok(InitializedProtocol::new(state, messages))
    }
}

/// The internal configuration of a EcdsSignConfig protocol.
struct EcdsSignConfig {
    eid: Vec<u8>,
    parties: Vec<PartyId>,
    private_key_shares: PartyShares<EcdsaPrivateKeyShare>,
    aux_info: PartyShares<EcdsaAuxInfo>,
    message_digest: [u8; 32],
}

fn verify(pk: EcdsaPublicKey, signature: EcdsaSignature, message: &DataToSign<Secp256k1>) -> bool {
    let EcdsaSignature { r, s } = signature;
    let cggmp_sig = Signature { r, s };

    let pk = pk.as_point();
    cggmp_sig.verify(pk, message).is_ok()
}

#[rstest]
fn end_to_end() {
    //0. Network configuration
    let max_rounds = 100;
    let network_size = 3;
    // 1. Message generation
    let message_to_sign = b"Transaction with plenty of bitcoin";
    let message_digest = DataToSign::digest::<Sha256>(message_to_sign);
    let message_digest_bytes =
        message_digest.to_scalar().to_be_bytes().as_bytes().try_into().expect("slice has to be 32 bytes long");
    // 2. Secret key generation
    let mut csprng = OsRng;
    let sk_val = SecretScalar::<Secp256k1>::random(&mut csprng);
    let sk: EcdsaPrivateKey = EcdsaPrivateKey::from_scalar(sk_val).unwrap();
    // 3. eid
    let eid = b"execution id, unique per protocol execution".to_vec();
    // 4. Run protocol
    let protocol = EcdsaSignProtocol::new(eid, sk.clone(), message_digest_bytes);
    let simulator = SymmetricProtocolSimulator::new(network_size, max_rounds);
    let outputs = simulator.run_protocol(&protocol).expect("protocol run failed");
    // 5. Collect shares
    let mut signature_shares = Vec::new();
    for output in outputs {
        match output.output {
            EcdsaSignatureShareOutput::Success { element: sig_share } => {
                signature_shares.push(sig_share);
            }
            EcdsaSignatureShareOutput::Abort { reason } => {
                panic!("Aborted with reason: {:?}", reason);
            }
        }
    }
    // 6. Reconstruct signature from shares
    let sig_reconstructed = EcdsaSignatureShare::recover(&signature_shares).unwrap();
    // 7. Verifies signature is valid under message_digest and public key
    let pk = EcdsaPublicKey::from_private_key(&sk);
    let verifies = verify(pk, sig_reconstructed, &message_digest);
    assert!(verifies)
}
