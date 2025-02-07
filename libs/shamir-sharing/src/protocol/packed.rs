//! Packed Secret Sharing Protocol

use crate::{
    party::{PartyMapper, TooManyParties},
    protocol::{EncoderError, RecoverSecretError, ShareGenerationError},
};
use basic_types::PartyId;
use math_lib::{
    decoders::{gao_decode, lagrange_polynomial},
    fields::Field,
    polynomial::{point::Point, point_sequence::PointSequence, Polynomial},
};

/// Shamir Secret Sharing Protocol
pub struct Packed<F>
where
    F: Field,
{
    /// The degree of the generated polynomial.
    pub(crate) polynomial_degree: u64,

    /// The number of secrets hidden in the polynomial.
    pub(crate) secret_count: u64,

    /// The type used to map parties to abscissas.
    pub(crate) mapper: PartyMapper<F>,

    /// Our own party id.
    pub(crate) local_party_id: PartyId,

    /// Abscissas used to hide secrets.
    pub(crate) locales: Vec<F::Inner>,
}

impl<F: Field> Packed<F> {
    /// Creates a new Shamir Secret Sharing Protocol.
    pub fn new(
        local_party_id: PartyId,
        polynomial_degree: u64,
        secret_count: u64,
        parties: Vec<PartyId>,
        locales: Vec<F::Inner>,
    ) -> Result<Self, EncoderError> {
        let mapper = PartyMapper::new(parties)?;
        let size = polynomial_degree.checked_add(1).ok_or(EncoderError::IntegerOverflow)?;
        if secret_count > size {
            return Err(EncoderError::TooManySecrets);
        }
        if locales.len() != (size as usize) {
            return Err(EncoderError::LocaleMismatch);
        }
        Ok(Self { polynomial_degree, secret_count, mapper, local_party_id, locales })
    }

    /// The number of parties in this configuration.
    pub fn party_count(&self) -> usize {
        self.mapper.party_count()
    }

    /// Gets the generated polynomial degree.
    pub fn polynomial_degree(&self) -> u64 {
        self.polynomial_degree
    }

    /// Gets the number of secrets in the generated polynomial.
    pub fn secret_count(&self) -> u64 {
        self.secret_count
    }

    /// Gets the party mapper.
    pub fn party_mapper(&self) -> &PartyMapper<F> {
        &self.mapper
    }

    /// Get our party id.
    pub fn local_party_id(&self) -> &PartyId {
        &self.local_party_id
    }

    /// Get the parties involved in this protocol.
    pub fn parties(&self) -> Vec<PartyId> {
        self.mapper.parties().cloned().collect()
    }

    /// Creates a new Polynomial Expression based on multiple secrets.
    pub fn build_polynomial(&self, secrets: &[F::Element]) -> Result<Polynomial<F>, EncoderError> {
        if secrets.len() != (self.secret_count as usize) {
            return Err(EncoderError::SecretCountMismatch);
        }
        let mut point_sequence = PointSequence::<F>::default();
        let mut locs = self.locales.iter();
        for (y, x) in secrets.iter().zip(locs.by_ref()) {
            point_sequence.push(Point::new(*x, *y));
        }
        for x in locs {
            if point_sequence.points().len() > (self.polynomial_degree as usize) {
                break;
            }
            let r = F::gen_random_element(&mut rand::thread_rng());
            point_sequence.push(Point::new(*x, r));
        }
        Ok(lagrange_polynomial(&point_sequence)?)
    }

    /// Generate the Shares from multiple secrets.
    pub fn generate_shares(&self, polynomial: &Polynomial<F>) -> Result<PointSequence<F>, ShareGenerationError> {
        let mut point_sequence = PointSequence::<F>::default();
        for x in self.mapper.abscissas() {
            let y = polynomial.eval_at(x)?;
            point_sequence.push(Point::new(*x, y))
        }
        Ok(point_sequence)
    }

    /// Recover the secrets from the given Shares.
    pub fn recover_secrets<I>(&self, shares: I) -> Result<Vec<F::Element>, RecoverSecretError>
    where
        I: Iterator<Item = (PartyId, F::Element)>,
    {
        let mut point_sequence = PointSequence::<F>::default();
        for (party_id, share) in shares {
            let x = self.mapper.abscissa(&party_id).ok_or(RecoverSecretError::PartyNotFound)?;
            let point = Point::new(*x, share);
            point_sequence.push(point);
        }
        self.recover_secrets_from_point_sequence(&point_sequence)
    }

    /// Recovers the secrets from a point sequence.
    pub fn recover_secrets_from_point_sequence(
        &self,
        point_sequence: &PointSequence<F>,
    ) -> Result<Vec<F::Element>, RecoverSecretError> {
        let poly = lagrange_polynomial(point_sequence)?;
        let mut secrets = Vec::new();
        for (i, x) in self.locales.iter().enumerate() {
            if i >= (self.secret_count as usize) {
                break;
            }
            secrets.push(poly.eval_at(x)?);
        }
        Ok(secrets)
    }

    /// Recover the polynomial from the given Shares.
    pub fn recover_polynomial<I>(&self, shares: I) -> Result<Polynomial<F>, RecoverSecretError>
    where
        I: Iterator<Item = (PartyId, F::Element)>,
    {
        let mut point_sequence = PointSequence::<F>::default();
        for (party_id, share) in shares {
            let x = self.mapper.abscissa(&party_id).ok_or(RecoverSecretError::PartyNotFound)?;
            let point = Point::new(*x, share);
            point_sequence.push(point);
        }
        let poly = lagrange_polynomial(&point_sequence)?;
        Ok(poly)
    }

    // TODO: handle slashing
    /// Recover the secret from the given Shares.
    pub fn robust_recover_secrets<I>(&self, shares: I) -> Result<Vec<F::Element>, RecoverSecretError>
    where
        I: Iterator<Item = (PartyId, F::Element)>,
    {
        let mut point_sequence = PointSequence::<F>::default();
        for (party_id, share) in shares {
            let x = self.mapper.abscissa(&party_id).ok_or(RecoverSecretError::PartyNotFound)?;
            let point = Point::new(*x, share);
            point_sequence.push(point);
        }
        self.robust_recover_secrets_from_point_sequence(&point_sequence)
    }

    /// Recovers the secrets from a point sequence using error correction.
    pub fn robust_recover_secrets_from_point_sequence(
        &self,
        point_sequence: &PointSequence<F>,
    ) -> Result<Vec<F::Element>, RecoverSecretError> {
        let poly_degree: usize = self.polynomial_degree as usize;
        let (poly, _error_poly) = gao_decode(point_sequence, poly_degree, poly_degree)?;
        let mut secrets = Vec::new();
        for (i, x) in self.locales.iter().enumerate() {
            if i >= (self.secret_count as usize) {
                break;
            }
            secrets.push(poly.eval_at(x)?);
        }
        Ok(secrets)
    }
}

impl<F, F2> TryFrom<&Packed<F>> for Packed<F2>
where
    F: Field,
    F2: Field<Inner = F::Inner>,
{
    type Error = TooManyParties;

    fn try_from(packed: &Packed<F>) -> Result<Self, Self::Error> {
        Ok(Packed {
            polynomial_degree: packed.polynomial_degree,
            secret_count: packed.secret_count,
            mapper: PartyMapper::new(packed.mapper.parties().cloned().collect())?,
            local_party_id: packed.local_party_id.clone(),
            locales: packed.locales.clone(),
        })
    }
}

#[allow(clippy::indexing_slicing, clippy::arithmetic_side_effects)]
#[cfg(test)]
mod test {
    use super::*;
    use math_lib::{
        fields::PrimeField,
        modular::{ModularNumber, U64SafePrime, U64},
    };

    type Prime = U64SafePrime;
    type Field = PrimeField<Prime>;

    /// Get default packed protocol for testing
    pub fn get_packed() -> Packed<Field> {
        // Create 531 party ids
        let party_ids = (100..1162).step_by(2).map(PartyId::from).collect();
        let local_party_id = PartyId::from(100);
        let locales = (1..12).map(|x| U64::from(200 - x as u32)).collect();
        Packed::new(local_party_id, 10u64, 5u64, party_ids, locales).unwrap()
    }

    /// Default behaviour of task
    pub fn test_packed(secrets: Vec<ModularNumber<Prime>>) {
        let packed = get_packed();
        let secret_poly = packed.build_polynomial(&secrets).unwrap();
        let shares = packed.generate_shares(&secret_poly).unwrap();
        let pool = shares.take(packed.polynomial_degree + 1).unwrap();
        let recovered_secrets = packed.recover_secrets_from_point_sequence(&pool).unwrap();
        assert_eq!(recovered_secrets, secrets, "Secret recovering failed!!");
    }

    #[test]
    fn test_packed_secrets() {
        let secrets = vec![
            ModularNumber::from_u32(30),
            ModularNumber::from_u32(19),
            ModularNumber::from_u32(9),
            ModularNumber::from_u32(8),
            ModularNumber::from_u32(22),
        ];
        test_packed(secrets);
    }

    #[test]
    fn test_packed_with_random_secrets() {
        let secrets = vec![
            ModularNumber::gen_random(),
            ModularNumber::gen_random(),
            ModularNumber::gen_random(),
            ModularNumber::gen_random(),
            ModularNumber::gen_random(),
        ];
        test_packed(secrets);
    }

    /// Fail when not enough shares test
    pub fn test_packed_fail(secrets: Vec<ModularNumber<Prime>>) {
        let packed = get_packed();
        let secret_poly = packed.build_polynomial(&secrets).unwrap();
        let shares = packed.generate_shares(&secret_poly).unwrap();
        let pool = shares.take(packed.polynomial_degree).unwrap();
        let recovered_secrets = packed.recover_secrets_from_point_sequence(&pool).unwrap();
        assert_ne!(recovered_secrets, secrets, "Secret recovered from T shares!!");
    }

    #[test]
    fn test_packed_fails() {
        let secrets = vec![
            ModularNumber::from_u32(30),
            ModularNumber::from_u32(19),
            ModularNumber::from_u32(9),
            ModularNumber::from_u32(8),
            ModularNumber::from_u32(22),
        ];
        test_packed_fail(secrets);
    }

    #[test]
    fn test_packed_fails_random_secret() {
        let secrets = vec![
            ModularNumber::gen_random(),
            ModularNumber::gen_random(),
            ModularNumber::gen_random(),
            ModularNumber::gen_random(),
            ModularNumber::gen_random(),
        ];
        test_packed_fail(secrets);
    }

    /// Robust reconstruct test.
    #[test]
    fn test_packed_ecc() {
        let secrets = vec![ModularNumber::from_u32(30), ModularNumber::from_u32(19), ModularNumber::from_u32(9)];

        let party_ids = (100..132).step_by(2).map(PartyId::from).collect();
        let local_party_id = PartyId::from(100);
        let locales = (1..7).map(|x| U64::from(200 - x as u32)).collect();

        let packed = Packed::<PrimeField<Prime>>::new(local_party_id, 5, 3, party_ids, locales).unwrap();
        let secret_poly = packed.build_polynomial(&secrets).unwrap();
        let shares = packed.generate_shares(&secret_poly).unwrap();

        // Corrupt shares
        let mut corrupted_shares = PointSequence::default();
        for i in 0..shares.points().len() {
            let (x, y) = &shares.points()[i].clone().into_coordinates();
            let mut z = *y;
            if i < packed.polynomial_degree as usize {
                z = ModularNumber::gen_random();
            }
            corrupted_shares.push(Point::new(*x, z));
        }

        // Recover Secret
        let recovered_secrets = packed.robust_recover_secrets_from_point_sequence(&corrupted_shares).unwrap();
        assert_eq!(recovered_secrets, secrets, "Secret robust recovering failed!!");
    }
}
