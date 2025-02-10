//! Shamir Secret Sharing Protocol

use crate::{
    party::PartyMapper,
    protocol::{HyperMapError, RecoverSecretError, ShamirError, ShareGenerationError},
};
use basic_types::PartyId;
use math_lib::{
    decoders::{gao_decode, lagrange_polynomial, Lagrange},
    errors::InterpolationError,
    fields::Field,
    matrix::Matrix,
    polynomial::{point::Point, point_sequence::PointSequence, Polynomial},
};

/// Shamir Secret Sharing Protocol
pub struct Shamir<F>
where
    F: Field,
{
    /// The degree of the generated polynomial.
    pub(crate) polynomial_degree: u64,

    /// The type used to map parties to abscissas.
    pub(crate) mapper: PartyMapper<F>,

    /// The interpolator used for secret recovery.
    pub(crate) lagrange: Lagrange<F>,

    /// The hyper-invertible matrix for randomness.
    pub(crate) matrix: Matrix<F>,

    /// Our own party id.
    pub(crate) local_party_id: PartyId,
}

/// Degree of polynomial.
#[derive(Clone, Copy)]
pub enum PolyDegree {
    /// Degree T equal to config polynomial degree.
    T,

    /// Degree 2T equal to double the config polynomial degree.
    TwoT,
}

impl<F: Field> Shamir<F> {
    /// Creates a new Shamir Secret Sharing Protocol.
    pub fn new(local_party_id: PartyId, polynomial_degree: u64, parties: Vec<PartyId>) -> Result<Self, ShamirError> {
        let len = u64::try_from(parties.len()).map_err(|_| ShamirError::Arithmetic)?;
        if len <= polynomial_degree {
            return Err(ShamirError::TooHighDegree);
        }
        let mapper = PartyMapper::<F>::new(parties)?;
        let mut abscissas = Vec::new();
        for x in mapper.abscissas() {
            abscissas.push(F::as_element(*x));
        }
        let nrows = u16::try_from(len.checked_sub(polynomial_degree).ok_or(ShamirError::Arithmetic)?)
            .map_err(|_| ShamirError::Arithmetic)?;
        let matrix = Matrix::vandermonde(&abscissas, nrows)?;
        let lagrange = Lagrange::new(abscissas)?;
        Ok(Self { polynomial_degree, mapper, lagrange, matrix, local_party_id })
    }

    /// The number of parties in this configuration.
    pub fn party_count(&self) -> usize {
        self.mapper.party_count()
    }

    /// Gets the generated polynomial degree.
    pub fn polynomial_degree(&self) -> u64 {
        self.polynomial_degree
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

    /// Generate the Shares from a secret.
    pub fn generate_shares(
        &self,
        secret: &F::Element,
        degree: PolyDegree,
    ) -> Result<PointSequence<F>, ShareGenerationError> {
        let mut polynomial = Polynomial::<F>::new(Vec::new());
        polynomial.add_coefficient(*secret);
        let degree = match degree {
            PolyDegree::T => self.polynomial_degree,
            PolyDegree::TwoT => self.polynomial_degree.wrapping_mul(2),
        };
        for _ in 0..degree {
            let coefficient = F::gen_random_element(&mut rand::thread_rng());
            polynomial.add_coefficient(coefficient);
        }

        let mut point_sequence = PointSequence::<F>::default();
        for x in self.mapper.abscissas() {
            let y = polynomial.eval_at(x)?;
            point_sequence.push(Point::new(*x, y))
        }
        Ok(point_sequence)
    }

    /// Return point sequence from shares.
    fn to_sequence<I>(&self, shares: I) -> Result<PointSequence<F>, RecoverSecretError>
    where
        I: Iterator<Item = (PartyId, F::Element)>,
    {
        let mut point_sequence = PointSequence::<F>::default();
        for (party_id, share) in shares {
            let x = self.mapper.abscissa(&party_id).ok_or(RecoverSecretError::PartyNotFound)?;
            let point = Point::new(*x, share);
            point_sequence.push(point);
        }
        Ok(point_sequence)
    }

    /// Recover the secret from the given Shares using any abscissas.
    pub fn explicit_recover_secret<I>(&self, shares: I) -> Result<F::Element, RecoverSecretError>
    where
        I: Iterator<Item = (PartyId, F::Element)>,
    {
        let point_sequence = self.to_sequence(shares)?;
        let secret = point_sequence.lagrange_interpolate()?;
        Ok(secret)
    }

    /// Recover the secret from the given Shares using same abscissas as initiation.
    pub fn recover_secret<I>(&self, shares: I) -> Result<F::Element, RecoverSecretError>
    where
        I: Iterator<Item = (PartyId, F::Element)>,
    {
        let point_sequence = self.to_sequence(shares)?;
        let secret = self.lagrange.interpolate(&point_sequence);
        if secret == Err(InterpolationError::MismatchedAbscissas) {
            let secret = point_sequence.lagrange_interpolate()?;
            return Ok(secret);
        }
        Ok(secret?)
    }

    /// Recover the polynomial from the given Shares.
    pub fn recover_polynomial<I>(&self, shares: I) -> Result<Polynomial<F>, RecoverSecretError>
    where
        I: Iterator<Item = (PartyId, F::Element)>,
    {
        let point_sequence = self.to_sequence(shares)?;
        let poly = lagrange_polynomial(&point_sequence)?;
        Ok(poly)
    }

    /// Recover the secret from the given Shares using error correction.
    pub fn robust_recover_secret<I>(&self, shares: I) -> Result<F::Element, RecoverSecretError>
    where
        I: Iterator<Item = (PartyId, F::Element)>,
    {
        let point_sequence = self.to_sequence(shares)?;
        let poly_degree: usize = self.polynomial_degree as usize;
        let (poly, _error_poly) = gao_decode(&point_sequence, poly_degree, poly_degree)?;
        let secret = poly.eval(&F::ZERO)?;
        Ok(secret)
    }

    /// Weigh the share using Lagrange coefficient.
    pub fn weigh(&self, share: &F::Element) -> Result<F::Element, RecoverSecretError> {
        let x = self.mapper.abscissa(&self.local_party_id).ok_or(RecoverSecretError::PartyNotFound)?;
        let output = self.lagrange.partial(x, share)?;
        Ok(output)
    }

    /// Hyper invertible mapping.
    pub fn hyper_map<I>(&self, shares: I) -> Result<Vec<F::Element>, HyperMapError>
    where
        I: Iterator<Item = (PartyId, F::Element)>,
    {
        let mut shares: Vec<_> = shares.into_iter().collect();
        shares.sort_by(|left, right| left.0.cmp(&right.0));
        let vector: Vec<_> = shares.into_iter().map(|(_, s)| s).collect();
        let ncols = u16::try_from(vector.len()).map_err(|_| HyperMapError::Arithmetic)?;
        let vector = Matrix::new(vector, 1, ncols)?;
        #[allow(clippy::arithmetic_side_effects)]
        let output = (vector * &self.matrix)?;
        Ok(output.to_vec())
    }
}

impl<F, F2> TryFrom<&Shamir<F>> for Shamir<F2>
where
    F: Field,
    F2: Field<Inner = F::Inner>,
{
    type Error = ShamirError;

    fn try_from(shamir: &Shamir<F>) -> Result<Self, Self::Error> {
        let shamir: Shamir<F2> = Shamir {
            polynomial_degree: shamir.polynomial_degree,
            mapper: PartyMapper::new(shamir.mapper.parties().cloned().collect())?,
            lagrange: Lagrange::try_from(&shamir.lagrange)?,
            matrix: Matrix::try_from(&shamir.matrix)?,
            local_party_id: shamir.local_party_id.clone(),
        };
        Ok(shamir)
    }
}

#[cfg(any(test, feature = "bench"))]
#[allow(clippy::unwrap_used, unused_imports, clippy::indexing_slicing, clippy::arithmetic_side_effects)]
pub mod test {
    //! Shamir Secret Sharing tests.
    use super::*;
    use basic_types::PartyId;
    use math_lib::{
        fields::PrimeField,
        modular::{ModularNumber, U64SafePrime, Uint},
        polynomial::{point::Point, point_sequence::PointSequence},
    };
    use std::collections::HashMap;

    type Prime = U64SafePrime;
    type Field = PrimeField<Prime>;

    /// Get default shamir protocol for testing
    pub fn get_shamir() -> Shamir<Field> {
        // Create 531 party ids
        let party_ids = (100..1162).step_by(2).map(PartyId::from).collect();
        let local_party_id = PartyId::from(100);
        Shamir::new(local_party_id, 37u64, party_ids).unwrap()
    }

    /// Default behaviour of task
    pub fn test(secret: ModularNumber<Prime>) {
        let shamir = get_shamir();
        let shares = shamir.generate_shares(&secret, PolyDegree::T).unwrap();
        let pool = shares.take(shamir.polynomial_degree + 1).unwrap();
        let recovered_secret: ModularNumber<_> = pool.lagrange_interpolate().unwrap();
        assert_eq!(recovered_secret, secret, "Secret recovering failed!!");
    }

    #[test]
    fn shamir_secret_15130512518() {
        test(ModularNumber::from_u64(15130512518));
    }

    #[test]
    fn shamir_with_random_secret() {
        test(ModularNumber::gen_random());
    }

    /// Fail when not enough shares test
    pub fn test_fail(secret: ModularNumber<Prime>) {
        let shamir = get_shamir();
        let shares = shamir.generate_shares(&secret, PolyDegree::T).unwrap();
        let pool = shares.take(shamir.polynomial_degree).unwrap();
        let recovered_secret: ModularNumber<_> = pool.lagrange_interpolate().unwrap();
        assert_ne!(recovered_secret, secret, "Secret recovered from T shares!!");
    }

    #[test]
    fn shamir_fails_when_not_enough_shares() {
        test_fail(ModularNumber::from_u32(123154213));
    }

    #[test]
    fn shamir_fails_random_secret() {
        test_fail(ModularNumber::gen_random());
    }

    #[test]
    fn grr_mult() {
        let n = 4;
        let parties: Vec<_> = (1..=n).map(|id| PartyId::from(id * 10)).collect();
        let shamir: Shamir<Field> = Shamir::new(parties[0].clone(), 1, parties.clone()).unwrap();

        // Share the secrets.
        let secret_1 = ModularNumber::from_u32(7);
        let shares_1 = shamir.generate_shares(&secret_1, PolyDegree::T).unwrap();

        let secret_2 = ModularNumber::from_u32(3);
        let shares_2 = shamir.generate_shares(&secret_2, PolyDegree::T).unwrap();

        // Mult Protocol.
        let mut sub_shares = Vec::new();
        for party in 0..shamir.party_count() {
            // Party multiplies the secrets.
            let left = shares_1.get_share(party).unwrap();
            let right = shares_2.get_share(party).unwrap();
            let local_product = left * &right;
            // Party sub-shares the local product.
            sub_shares.push(shamir.generate_shares(&local_product, PolyDegree::T).unwrap());
        }

        let mut product_shares = HashMap::new();
        for party in 0..shamir.party_count() {
            // Party gets shares from all nodes.
            let mut party_shares = HashMap::new();
            for other in 0..shamir.party_count() {
                let other_shares = sub_shares.get(other).unwrap();
                // Party gets their share from other.
                let share = other_shares.get_share(party).unwrap();
                party_shares.insert(parties[other].clone(), share);
            }
            // Party recovers their share of the product.
            let product_share = shamir.recover_secret(party_shares.into_iter()).unwrap();
            product_shares.insert(parties[party].clone(), product_share);
        }
        // Done with Mult protocol.

        // Check: Reconstruct the product of secrets.
        let recovered_product = shamir.recover_secret(product_shares.into_iter()).unwrap();

        let expected = ModularNumber::from_u32(21);
        assert_eq!(recovered_product, expected);
    }

    /// Robust reconstruct test.
    #[test]
    fn ecc_works() {
        let secret = ModularNumber::from_u32(212839);

        let party_ids = (100..132).step_by(2).map(PartyId::from).collect();
        let local_party_id = PartyId::from(100);
        let poly_degree = 5;

        let shamir = Shamir::<Field>::new(local_party_id, poly_degree, party_ids).unwrap();
        let shares = shamir.generate_shares(&secret, PolyDegree::T).unwrap();

        // Corrupt shares
        let mut corrupted_shares = PointSequence::<Field>::default();
        for i in 0..shares.points().len() {
            let (x, y) = &shares.points()[i].clone().into_coordinates();
            let mut z = *y;
            if i < shamir.polynomial_degree as usize {
                z = ModularNumber::gen_random();
            }
            corrupted_shares.push(Point::new(*x, z));
        }

        // Recover Secret
        let (poly, _error_poly) = gao_decode(&corrupted_shares, poly_degree as usize, poly_degree as usize).unwrap();
        let recovered_secret: ModularNumber<_> = poly.eval(&ModularNumber::ZERO).unwrap();
        assert_eq!(recovered_secret, secret, "Secret robust recovering failed!!");
    }
}
