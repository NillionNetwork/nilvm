#![allow(clippy::arithmetic_side_effects, clippy::panic, clippy::indexing_slicing)]

use super::{output::KeyGenOutput, CurveProtocol, Ed25519Protocol, Secp256k1Protocol};
use crate::{
    distributed_key_generation::dkg::KeyGenState,
    simulator::symmetric::{InitializedProtocol, Protocol, SymmetricProtocolSimulator},
};
use anyhow::{Error, Result};
use basic_types::PartyId;
use threshold_keypair::privatekey::ThresholdPrivateKeyShare;

use cggmp21::generic_ec::{
    curves::{Ed25519, Secp256k1},
    Curve, Point,
};

struct KeyGenProtocol<C: CurveProtocol> {
    eid: Vec<u8>,
    _phantom: std::marker::PhantomData<C>,
}

impl<C: CurveProtocol> KeyGenProtocol<C> {
    fn new(eid: Vec<u8>) -> Self {
        Self { eid, _phantom: std::marker::PhantomData }
    }
}

impl<C: CurveProtocol> Protocol for KeyGenProtocol<C> {
    type State = KeyGenState<C>;
    type PrepareOutput = KeyGenConfig;

    fn prepare(&self, parties: &[PartyId]) -> Result<Self::PrepareOutput, Error> {
        Ok(KeyGenConfig { eid: self.eid.clone(), parties: parties.to_vec() })
    }

    fn initialize(
        &self,
        party_id: PartyId,
        config: &Self::PrepareOutput,
    ) -> Result<InitializedProtocol<Self::State>, anyhow::Error> {
        let (state, messages) = KeyGenState::new(config.eid.clone(), config.parties.clone(), party_id)?;

        Ok(InitializedProtocol::new(state, messages))
    }
}

/// The internal configuration of a KeyGen protocol.
struct KeyGenConfig {
    eid: Vec<u8>,
    parties: Vec<PartyId>,
}

/// This function was taken from cggmp21 tests to validate key shares as they do not have an exposed
/// function in their library to execute that validation. It verifies that:
/// - Each share has correct index and matching public info
/// - Generator * private share equals corresponding public share
/// - Reconstructed secret key matches the public key
fn validate_key_shares<C: Curve>(private_key_shares: &[ThresholdPrivateKeyShare<C>]) {
    // Get the first key share to compare against
    let first_share = private_key_shares[0].as_inner();
    // Sort private_key_shares by the i component of the inner share
    let mut sorted_shares = private_key_shares.to_vec();
    sorted_shares.sort_by_key(|share| share.as_inner().i);
    let sorted_private_key_shares = sorted_shares;

    // Validate each share has correct index and matching public info
    for (i, key_share) in (0u16..).zip(sorted_private_key_shares.clone()) {
        let share = key_share.as_inner();
        // assert_eq!(share.i, i);
        assert_eq!(share.shared_public_key, first_share.shared_public_key);
        assert_eq!(share.public_shares, first_share.public_shares);

        // Verify that generator * private share equals the corresponding public share
        assert_eq!(Point::<C>::generator() * &share.x, share.public_shares[usize::from(i)]);
    }

    // Reconstruct using all shares
    let all_shares: Vec<_> = sorted_private_key_shares.iter().map(|share| share.clone().into_inner()).collect();

    // Reconstruct secret key and verify it matches the public key
    let sk = key_share::reconstruct_secret_key(&all_shares).unwrap();
    assert_eq!(Point::<C>::generator() * sk, first_share.shared_public_key);
}

#[test]
fn end_to_end_ecdsa() {
    //0. Network configuration
    let max_rounds = 100;
    let network_size = 3;
    // 1. eid
    let eid = b"execution id, unique per protocol execution".to_vec();
    // 2. Run protocol
    let protocol = KeyGenProtocol::<Secp256k1Protocol>::new(eid);
    let simulator = SymmetricProtocolSimulator::new(network_size, max_rounds);
    let outputs = simulator.run_protocol(&protocol).expect("protocol run failed");
    // 3. Collect shares
    let mut private_key_shares = Vec::new();
    for output in outputs {
        match output.output {
            KeyGenOutput::Success { element: key_share } => {
                private_key_shares.push(key_share);
            }
            KeyGenOutput::Abort { reason } => {
                panic!("Aborted with reason: {:?}", reason);
            }
        }
    }
    // 4. Validate key shares
    validate_key_shares::<Secp256k1>(&private_key_shares);
}

#[test]
fn end_to_end_eddsa() {
    //0. Network configuration
    let max_rounds = 100;
    let network_size = 3;
    // 1. eid
    let eid = b"execution id, unique per protocol execution".to_vec();
    // 2. Run protocol
    let protocol = KeyGenProtocol::<Ed25519Protocol>::new(eid);
    let simulator = SymmetricProtocolSimulator::new(network_size, max_rounds);
    let outputs = simulator.run_protocol(&protocol).expect("protocol run failed");
    // 3. Collect shares
    let mut private_key_shares = Vec::new();
    for output in outputs {
        match output.output {
            KeyGenOutput::Success { element: key_share } => {
                private_key_shares.push(key_share);
            }
            KeyGenOutput::Abort { reason } => {
                panic!("Aborted with reason: {:?}", reason);
            }
        }
    }
    // 4. Validate key shares
    validate_key_shares::<Ed25519>(&private_key_shares);
}
