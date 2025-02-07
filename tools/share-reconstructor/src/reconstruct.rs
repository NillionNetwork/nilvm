use crate::config::{PartyConfig, PrimeFieldConfig, SemiFieldConfig};
use anyhow::{anyhow, Result};
use math_lib::{
    galois::GF256,
    modular::{ModularNumber, SafePrime},
    ring::RingTuple,
};
use shamir_sharing::{
    party::PartyId,
    secret_sharer::{PartyShares, SecretSharer, ShamirSecretSharer},
};

/// Performs reconstruction of shares into the secret behind them.
#[derive(Default)]
pub struct Reconstructor;

impl Reconstructor {
    /// Reconstructs the given shares using the provided prime as `P`.
    pub fn reconstruct<T, U, O>(&self, shares: Vec<PartyConfig<U>>) -> Result<O>
    where
        T: SafePrime,
        U: IntoShare<T>,
        ShamirSecretSharer<T>: SecretSharer<<U as IntoShare<T>>::Share, Secret = O>,
    {
        // Dummy as we're not hiding anything and are using all shares to reconstruct
        let polynomial_degree = 1;
        let parties: Vec<_> = shares.iter().map(|share| PartyId::from(share.party_id)).collect();
        let secret_sharer = ShamirSecretSharer::new(parties[0].clone(), polynomial_degree, parties)?;
        let mut party_shares: PartyShares<U::Share> = PartyShares::default();
        for share in shares {
            let party_id = PartyId::from(share.party_id.as_bytes().to_vec());
            party_shares.insert(party_id, share.share.into_share());
        }
        let secret = secret_sharer.recover(party_shares).map_err(|e| anyhow!("reconstruction failed: {e}"))?;
        Ok(secret)
    }
}

/// Turns something into a share.
pub trait IntoShare<T: SafePrime> {
    type Share;

    /// Turn this something into a share using the provided prime number as `P`.
    fn into_share(self) -> Self::Share;
}

impl<T> IntoShare<T> for PrimeFieldConfig
where
    T: SafePrime,
{
    type Share = ModularNumber<T>;

    fn into_share(self) -> Self::Share {
        // TODO
        ModularNumber::try_from(&self.element).unwrap()
    }
}

impl<T> IntoShare<T> for SemiFieldConfig
where
    T: SafePrime,
{
    type Share = RingTuple<T::SophiePrime>;

    fn into_share(self) -> Self::Share {
        // TODO
        let prime_element = ModularNumber::try_from(&self.prime_element).unwrap();
        let binary_ext_element = GF256::new(self.binary_ext_element);
        RingTuple::new(prime_element, binary_ext_element)
    }
}
