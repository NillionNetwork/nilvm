//! Secret sharer implementation.

use super::SecretSharerProperties;
use crate::{
    party::PartyMapper,
    protocol::{HyperMapError, PolyDegree, RecoverSecretError, Shamir, ShamirError},
    secret_sharer::{GenerateSharesError, MultiMapError, MultiRecoverError, PartyShares, SecretSharer},
};
use basic_types::PartyId;
use math_lib::{
    fields::{BinaryExtField, Field, PrimeField},
    galois::GF256,
    modular::{ConditionallySelectable, Integer, ModularNumber, SafePrime},
    polynomial::point::Point,
    ring::{crt, RingTuple},
};
use std::sync::Arc;

/// A secret sharer that abstracts the specific Shamir implementation being used.
#[derive(Clone)]
pub struct ShamirSecretSharer<T: SafePrime> {
    local_party_id: PartyId,
    binary_ext_shamir: Arc<Shamir<BinaryExtField>>,
    shamir: Arc<Shamir<PrimeField<T>>>,
    sophie_shamir: Arc<Shamir<PrimeField<T::SophiePrime>>>,
}

impl<T: SafePrime> ShamirSecretSharer<T> {
    /// The prime number this sharer is operating on.
    pub const PRIME: T::Normal = T::MODULO;

    /// Constructs a new secret sharer.
    ///
    /// The provided prime must be a safe prime. No checks are performed to validate it is the user's responsibility to
    /// pick the right prime.
    ///
    /// # Arguments
    /// * `local_party_id` - Our party id.
    /// * `polynomial_degree` - The degree of the generated polynomials.
    /// * `parties` - The list of parties id that the shares will be generated for. This will affect the number of
    ///   generated shares as well as the abscissas used during the evaluations of the polynomial.
    /// * `prime` - The safe prime to be used.
    pub fn new(local_party_id: PartyId, polynomial_degree: u64, parties: Vec<PartyId>) -> Result<Self, ShamirError> {
        let shamir = Arc::new(Shamir::new(local_party_id.clone(), polynomial_degree, parties.clone())?);
        let sophie_shamir = Arc::new(Shamir::new(local_party_id.clone(), polynomial_degree, parties.clone())?);
        let binary_ext_shamir = Arc::new(Shamir::new(local_party_id.clone(), polynomial_degree, parties)?);
        Ok(Self { local_party_id, shamir, sophie_shamir, binary_ext_shamir })
    }

    fn generate_field_shares<F: Field>(
        secret: &F::Element,
        degree: PolyDegree,
        shamir: &Shamir<F>,
    ) -> Result<PartyShares<F::Element>, GenerateSharesError> {
        let points = shamir.generate_shares(secret, degree)?.into_points();
        let mut party_shares = PartyShares::with_capacity_and_hasher(points.len(), Default::default());
        for point in points {
            let (x, share) = point.into_coordinates();
            let party_id = shamir.party_mapper().party(&x).ok_or(GenerateSharesError::AbscissaMapping)?;
            party_shares.insert(party_id.clone(), share);
        }
        Ok(party_shares)
    }
}

impl<T: SafePrime> SecretSharerProperties for ShamirSecretSharer<T> {
    type Prime = T;

    fn local_party_id(&self) -> &PartyId {
        &self.local_party_id
    }

    fn parties(&self) -> Vec<PartyId> {
        self.shamir.parties()
    }

    fn party_count(&self) -> usize {
        self.shamir.party_count()
    }

    fn party_mapper(&self) -> &PartyMapper<PrimeField<Self::Prime>> {
        self.shamir.party_mapper()
    }

    fn polynomial_degree(&self) -> u64 {
        self.shamir.polynomial_degree
    }
}

// Defines a `SecretSharer` for a particular prime number.
//
// This is needed as otherwise defining this generically for a `UintSafePrime` and its
// `UintSafePrime::SOPHIE_PRIME` would make the trait definition ambiguous. The prime types we use
// is pre-defined anyway so in practice this doesn't change anything.
macro_rules! impl_modular_secret_sharer {
    ($sharer_base_type:ty, $prime_type:ty, $shamir:tt) => {
        impl $crate::secret_sharer::SecretSharer<math_lib::modular::ModularNumber<$prime_type>>
            for $crate::secret_sharer::ShamirSecretSharer<$sharer_base_type>
        {
            type Secret = math_lib::modular::ModularNumber<$prime_type>;
            type RecoverError = $crate::protocol::RecoverSecretError;
            type HyperMapError = $crate::protocol::HyperMapError;

            fn generate_shares(
                &self,
                secret: &Self::Secret,
                degree: PolyDegree,
            ) -> Result<
                $crate::secret_sharer::PartyShares<math_lib::modular::ModularNumber<$prime_type>>,
                $crate::secret_sharer::GenerateSharesError,
            > {
                Self::generate_field_shares(secret, degree, &self.$shamir)
            }

            fn recover<I>(&self, shares: I) -> Result<Self::Secret, Self::RecoverError>
            where
                I: IntoIterator<Item = ($crate::party::PartyId, math_lib::modular::ModularNumber<$prime_type>)>,
            {
                self.$shamir.recover_secret(shares.into_iter())
            }

            fn weigh(
                &self,
                share: math_lib::modular::ModularNumber<$prime_type>,
            ) -> Result<Self::Secret, Self::RecoverError> {
                self.$shamir.weigh(&share)
            }

            fn hyper_map<I>(&self, shares: I) -> Result<Vec<Self::Secret>, Self::HyperMapError>
            where
                I: IntoIterator<Item = ($crate::party::PartyId, math_lib::modular::ModularNumber<$prime_type>)>,
            {
                self.$shamir.hyper_map(shares.into_iter())
            }
        }
    };
    ($type:ty) => {
        impl_modular_secret_sharer!($type, $type, shamir);
    };
}

// Secret sharing of a secret mod p.
impl_modular_secret_sharer!(math_lib::modular::U64SafePrime);
impl_modular_secret_sharer!(math_lib::modular::U128SafePrime);
impl_modular_secret_sharer!(math_lib::modular::U256SafePrime);

// Secret sharing of a secret mod q
impl_modular_secret_sharer!(math_lib::modular::U64SafePrime, math_lib::modular::U64SophiePrime, sophie_shamir);
impl_modular_secret_sharer!(math_lib::modular::U128SafePrime, math_lib::modular::U128SophiePrime, sophie_shamir);
impl_modular_secret_sharer!(math_lib::modular::U256SafePrime, math_lib::modular::U256SophiePrime, sophie_shamir);

impl<T: SafePrime> SecretSharer<GF256> for ShamirSecretSharer<T> {
    type Secret = GF256;
    type RecoverError = RecoverSecretError;
    type HyperMapError = HyperMapError;

    fn generate_shares(
        &self,
        secret: &Self::Secret,
        degree: PolyDegree,
    ) -> Result<PartyShares<GF256>, GenerateSharesError> {
        Self::generate_field_shares(secret, degree, &self.binary_ext_shamir)
    }

    fn recover<I>(&self, shares: I) -> Result<Self::Secret, Self::RecoverError>
    where
        I: IntoIterator<Item = (PartyId, GF256)>,
    {
        self.binary_ext_shamir.recover_secret(shares.into_iter())
    }

    fn weigh(&self, share: GF256) -> Result<Self::Secret, Self::RecoverError> {
        self.binary_ext_shamir.weigh(&share)
    }

    fn hyper_map<I>(&self, shares: I) -> Result<Vec<Self::Secret>, Self::HyperMapError>
    where
        I: IntoIterator<Item = (PartyId, GF256)>,
    {
        self.binary_ext_shamir.hyper_map(shares.into_iter())
    }
}

impl<T: SafePrime> SecretSharer<RingTuple<T::SophiePrime>> for ShamirSecretSharer<T> {
    type Secret = ModularNumber<T::SemiPrime>;
    type RecoverError = RecoverSecretError;
    type HyperMapError = HyperMapError;

    fn generate_shares(
        &self,
        secret: &Self::Secret,
        degree: PolyDegree,
    ) -> Result<PartyShares<RingTuple<T::SophiePrime>>, GenerateSharesError> {
        let secret = secret.into_value();
        // We only care about the last bit, the rest are random.
        let binary_ext_secret = GF256::gen_random();
        let binary_ext_secret = u8::conditional_select(
            &(binary_ext_secret.value() | 0x01),
            &(binary_ext_secret.value() & 0xfe),
            secret.is_even(),
        );
        let binary_ext_secret = GF256::new(binary_ext_secret);
        let binary_ext_points = self.binary_ext_shamir.generate_shares(&binary_ext_secret, degree)?.into_points();
        let secret = ModularNumber::<T::SophiePrime>::new(secret);
        let prime_points: Vec<Point<PrimeField<T::SophiePrime>>> =
            self.sophie_shamir.generate_shares(&secret, degree)?.into_points();
        let mut party_shares = PartyShares::with_capacity_and_hasher(prime_points.len(), Default::default());
        for (prime_point, binary_ext_point) in prime_points.into_iter().zip(binary_ext_points.into_iter()) {
            let (x, prime_share) = prime_point.into_coordinates();
            let (_, binary_ext_share) = binary_ext_point.into_coordinates();
            let party_id = self.sophie_shamir.party_mapper().party(&x).ok_or(GenerateSharesError::AbscissaMapping)?;
            party_shares.insert(party_id.clone(), RingTuple::new(prime_share, binary_ext_share));
        }
        Ok(party_shares)
    }

    fn recover<I>(&self, shares: I) -> Result<Self::Secret, Self::RecoverError>
    where
        I: IntoIterator<Item = (PartyId, RingTuple<T::SophiePrime>)>,
    {
        let (prime_shares, binary_ext_shares): (Vec<_>, Vec<_>) = shares
            .into_iter()
            .map(|(party_id, element)| {
                let (prime_element, binary_ext_element) = element.into_parts();
                ((party_id.clone(), prime_element), (party_id, binary_ext_element))
            })
            .unzip();
        let prime_secret = self.sophie_shamir.recover_secret(prime_shares.into_iter())?;
        let binary_ext_secret = self.binary_ext_shamir.recover_secret(binary_ext_shares.into_iter())?;
        let secret = crt(RingTuple::new(prime_secret, binary_ext_secret));
        Ok(secret)
    }

    fn weigh(&self, share: RingTuple<T::SophiePrime>) -> Result<Self::Secret, Self::RecoverError> {
        let (prime_element, binary_ext_element) = share.into_parts();
        let weighed_sophie = self.sophie_shamir.weigh(&prime_element)?;
        let weighed_binary_ext = self.binary_ext_shamir.weigh(&binary_ext_element)?;
        let weighed_share = crt(RingTuple::new(weighed_sophie, weighed_binary_ext));
        Ok(weighed_share)
    }

    fn hyper_map<I>(&self, shares: I) -> Result<Vec<Self::Secret>, Self::HyperMapError>
    where
        I: IntoIterator<Item = (PartyId, RingTuple<T::SophiePrime>)>,
    {
        let (prime_shares, binary_ext_shares): (Vec<_>, Vec<_>) = shares
            .into_iter()
            .map(|(party_id, element)| {
                let (prime_element, binary_ext_element) = element.into_parts();
                ((party_id.clone(), prime_element), (party_id, binary_ext_element))
            })
            .unzip();
        let prime_output = self.sophie_shamir.hyper_map(prime_shares.into_iter())?;
        let binary_ext_output = self.binary_ext_shamir.hyper_map(binary_ext_shares.into_iter())?;
        let mut outputs = Vec::new();
        for (p, b) in prime_output.into_iter().zip(binary_ext_output.into_iter()) {
            let out = crt(RingTuple::new(p, b));
            outputs.push(out);
        }

        Ok(outputs)
    }
}

impl<S, T> SecretSharer<Vec<S>> for ShamirSecretSharer<T>
where
    Self: SecretSharer<S>,
    S: Clone,
    T: SafePrime,
{
    type Secret = Vec<<Self as SecretSharer<S>>::Secret>;
    type RecoverError = MultiRecoverError<<Self as SecretSharer<S>>::RecoverError>;
    type HyperMapError = MultiMapError<<Self as SecretSharer<S>>::HyperMapError>;

    fn generate_shares(
        &self,
        secrets: &Self::Secret,
        degree: PolyDegree,
    ) -> Result<PartyShares<Vec<S>>, GenerateSharesError> {
        let mut party_shares: PartyShares<Vec<S>> = PartyShares::default();
        for secret in secrets {
            let shares = self.generate_shares(secret, degree)?;
            for (party_id, share) in shares {
                party_shares.entry(party_id).or_insert_with(|| Vec::with_capacity(secrets.len())).push(share);
            }
        }
        Ok(party_shares)
    }

    fn recover<I>(&self, shares: I) -> Result<Self::Secret, Self::RecoverError>
    where
        I: IntoIterator<Item = (PartyId, Vec<S>)>,
    {
        let mut shares = shares.into_iter();
        let first = shares.next().ok_or(MultiRecoverError::NoShares)?;
        let mut secret_shares = vec![PartyShares::<S>::default(); first.1.len()];
        for (party_id, shares) in std::iter::once(first).chain(shares) {
            if shares.len() != secret_shares.len() {
                return Err(MultiRecoverError::ShareCountMismatch);
            }
            for (share, secret_shares) in shares.into_iter().zip(secret_shares.iter_mut()) {
                secret_shares.insert(party_id.clone(), share);
            }
        }
        let mut secrets = Vec::new();
        for shares in secret_shares {
            let secret = self.recover(shares)?;
            secrets.push(secret);
        }
        Ok(secrets)
    }

    fn weigh(&self, shares: Vec<S>) -> Result<Self::Secret, Self::RecoverError> {
        let mut weighed_shares = Vec::new();
        for share in shares.into_iter() {
            let weighed_share = self.weigh(share)?;
            weighed_shares.push(weighed_share);
        }
        Ok(weighed_shares)
    }

    fn hyper_map<I>(&self, shares: I) -> Result<Vec<Self::Secret>, Self::HyperMapError>
    where
        I: IntoIterator<Item = (PartyId, Vec<S>)>,
    {
        let mut shares = shares.into_iter();
        let first = shares.next().ok_or(MultiMapError::NoShares)?;
        let mut secret_shares = vec![PartyShares::<S>::default(); first.1.len()];
        for (party_id, shares) in std::iter::once(first).chain(shares) {
            if shares.len() != secret_shares.len() {
                return Err(MultiMapError::ShareCountMismatch);
            }
            for (share, secret_shares) in shares.into_iter().zip(secret_shares.iter_mut()) {
                secret_shares.insert(party_id.clone(), share);
            }
        }
        let mut outputs = Vec::new();
        for shares in secret_shares {
            let output = self.hyper_map(shares)?;
            outputs.push(output);
        }
        Ok(outputs)
    }
}

/// The sharer is not operating over a safe prime field.
#[derive(thiserror::Error, Debug)]
#[error("not a safe prime field")]
pub struct NotSafePrimeError;

/// Creates a secret sharer for testing purposes
#[cfg(any(test, feature = "testing"))]
pub fn test_secret_sharer<T: SafePrime>() -> ShamirSecretSharer<T> {
    let local_party_id = PartyId::from(10);
    let parties = vec![local_party_id.clone(), PartyId::from(20), PartyId::from(30)];
    ShamirSecretSharer::<T>::new(local_party_id, 1, parties).expect("build secret sharer failed")
}

#[allow(clippy::indexing_slicing)]
#[cfg(test)]
mod test {
    use super::*;
    use math_lib::modular::{U64SafePrime, U64SemiPrime, U64SophiePrime};
    use rstest::rstest;

    type Prime = U64SafePrime;
    type SemiPrime = U64SemiPrime;
    type SophiePrime = U64SophiePrime;

    #[rstest]
    #[case(1)]
    #[case(42)]
    #[case(43)]
    #[case(15000)]
    #[case(20000)]
    #[case(22321)]
    #[case(23098)]
    #[test]
    fn modular_sharing(#[case] secret: u32) {
        let parties = vec![PartyId::from(1), PartyId::from(2), PartyId::from(3)];
        let sharer = ShamirSecretSharer::<Prime>::new(parties[0].clone(), 1, parties).unwrap();

        let secret = ModularNumber::<Prime>::from_u32(secret);
        let shares: PartyShares<ModularNumber<Prime>> = sharer.generate_shares(&secret, PolyDegree::T).unwrap();
        assert_eq!(shares.len(), 3);
        let recovered_secret = sharer.recover(shares).unwrap();
        assert_eq!(recovered_secret, secret);
    }

    #[rstest]
    #[case(1)]
    #[case(42)]
    #[case(43)]
    #[case(15000)]
    #[case(20000)]
    #[case(22321)]
    #[case(23098)]
    #[test]
    fn sophie_modular_sharing(#[case] secret: u32) {
        let parties = vec![PartyId::from(1), PartyId::from(2), PartyId::from(3)];
        let sharer = ShamirSecretSharer::<Prime>::new(parties[0].clone(), 1, parties).unwrap();

        let secret = ModularNumber::<SophiePrime>::from_u32(secret);
        let shares: PartyShares<ModularNumber<SophiePrime>> = sharer.generate_shares(&secret, PolyDegree::T).unwrap();
        assert_eq!(shares.len(), 3);
        let recovered_secret = sharer.recover(shares).unwrap();
        assert_eq!(recovered_secret, secret);
    }

    #[rstest]
    #[case(1)]
    #[case(2)]
    #[case(42)]
    #[case(43)]
    #[case(220)]
    #[case(254)]
    #[test]
    fn gf256_sharing(#[case] secret: u8) {
        let parties = vec![PartyId::from(1), PartyId::from(2), PartyId::from(3)];
        let sharer = ShamirSecretSharer::<Prime>::new(parties[0].clone(), 1, parties).unwrap();

        let secret = GF256::new(secret);
        let shares: PartyShares<GF256> = sharer.generate_shares(&secret, PolyDegree::T).unwrap();
        assert_eq!(shares.len(), 3);
        let recovered_secret = sharer.recover(shares).unwrap();
        assert_eq!(recovered_secret, secret);
    }

    #[rstest]
    #[case(1)]
    #[case(42)]
    #[case(43)]
    #[case(15000)]
    #[case(20000)]
    #[case(22321)]
    #[case(23098)]
    #[test]
    fn semi_field_sharing(#[case] secret: u32) {
        let parties = vec![PartyId::from(1), PartyId::from(2), PartyId::from(3)];
        let sharer = ShamirSecretSharer::<Prime>::new(parties[0].clone(), 1, parties).unwrap();

        let secret = ModularNumber::<SemiPrime>::from_u32(secret);
        let shares: PartyShares<RingTuple<SophiePrime>> = sharer.generate_shares(&secret, PolyDegree::T).unwrap();
        assert_eq!(shares.len(), 3);
        let recovered_secret = sharer.recover(shares).unwrap();
        assert_eq!(recovered_secret, secret);
    }

    #[test]
    fn bulk_sharing() {
        let parties = vec![PartyId::from(1), PartyId::from(2), PartyId::from(3)];
        let sharer = ShamirSecretSharer::<Prime>::new(parties[0].clone(), 1, parties).unwrap();

        let secrets = vec![ModularNumber::from_u32(42), ModularNumber::from_u32(1337)];
        let shares: PartyShares<Vec<ModularNumber<Prime>>> = sharer.generate_shares(&secrets, PolyDegree::T).unwrap();
        assert_eq!(shares.len(), 3);
        let recovered_secrets = sharer.recover(shares).unwrap();
        assert_eq!(recovered_secrets, secrets);
    }
}
