//! Node keys helpers.

use anyhow::Result;
use basic_types::PartyId;
use user_keypair::{secp256k1::Secp256k1SigningKey, SigningKey};

/// The identities of all the nodes in the cluster.
pub struct NodeIdentities(pub Vec<NodeIdentity>);

impl NodeIdentities {
    /// Construct a new set of identities using a seed.
    pub fn new(base_seed: &str, count: u32) -> Result<Self> {
        let seeds: Vec<_> = (0..count).map(|index| format!("{base_seed}:{index}")).collect();
        let keys = seeds
            .iter()
            .map(|seed| Secp256k1SigningKey::try_from_seed(seed).map(SigningKey::from))
            .collect::<Result<Vec<_>, _>>()?;
        let party_ids = keys.iter().map(|key| PartyId::from(key.public_key().as_bytes().to_vec())).collect::<Vec<_>>();

        let elements = keys.into_iter().zip(party_ids);
        let identities = elements.map(|(key, party_id)| NodeIdentity { key, party_id }).collect();
        Ok(Self(identities))
    }
}

/// A node's identity.
pub struct NodeIdentity {
    /// The node's key.
    pub key: SigningKey,

    /// The node's party id.
    pub party_id: PartyId,
}
