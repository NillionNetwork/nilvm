//! Anything related to a beaver triple.

use math_lib::{
    errors::DivByZero,
    modular::{Modular, ModularNumber, Prime},
};

/// Shares of a triple of numbers such that `result = left_operand * right_operand`.
#[derive(Clone)]
pub struct BeaverTriple<T: Modular> {
    /// A share of the left operand used in the product.
    pub left_operand: ModularNumber<T>,

    /// A share of the right operand used in the product.
    pub right_operand: ModularNumber<T>,

    /// A share of the result of the product.
    pub result: ModularNumber<T>,
}

impl<T: Prime> BeaverTriple<T> {
    /// Constructs a new beaver triple. It's the creator's responsibility to check whether the properties
    /// of the provided numbers hold.
    pub fn new(left_operand: ModularNumber<T>, right_operand: ModularNumber<T>, result: ModularNumber<T>) -> Self {
        Self { left_operand, right_operand, result }
    }

    /// Prepare the multiplication of `left_operand` with `right_operand` using this triple. This returns
    /// a share of each of the left/right operands masked with the triples accordingly.
    pub fn prepare_multiplication(
        &self,
        left_operand: &ModularNumber<T>,
        right_operand: &ModularNumber<T>,
    ) -> Result<(ModularNumber<T>, ModularNumber<T>), DivByZero> {
        let left_operand = left_operand - &self.left_operand;
        let right_operand = right_operand - &self.right_operand;
        Ok((left_operand, right_operand))
    }

    /// Finalize the multiplication using the scalars that resulted out of reconstructing the
    /// masked operands created during the "prepare" phase.
    pub fn finalize_multiplication(
        &self,
        left_scalar: &ModularNumber<T>,
        right_scalar: &ModularNumber<T>,
    ) -> Result<ModularNumber<T>, DivByZero> {
        let output = left_scalar * &self.right_operand;
        let output = output + &(right_scalar * &self.left_operand);
        let output = output + &self.result;
        let output = output + &(left_scalar * right_scalar);
        Ok(output)
    }
}

#[allow(clippy::indexing_slicing)]
#[cfg(test)]
mod test {
    use super::*;
    use anyhow::Result;
    use math_lib::{fields::PrimeField, modular::U64SafePrime, polynomial::point_sequence::PointSequence};
    use shamir_sharing::{
        party::PartyId,
        protocol::{PolyDegree, Shamir},
    };
    use std::sync::Arc;

    type Prime = U64SafePrime;
    type U64Field = PrimeField<Prime>;

    fn make_shamir() -> Arc<Shamir<PrimeField<Prime>>> {
        let parties = vec![PartyId::from(10), PartyId::from(20)];
        let shamir = Shamir::new(parties[0].clone(), 1, parties).unwrap();
        Arc::new(shamir)
    }

    // Constructs shares of the elements in a Beaver triple and returns a `BeaverTriple` for every party.
    fn make_triples(
        left: &ModularNumber<Prime>,
        right: &ModularNumber<Prime>,
        shamir: &Shamir<PrimeField<Prime>>,
    ) -> Result<Vec<BeaverTriple<Prime>>> {
        let result = left * right;
        let shares_left = shamir.generate_shares(left, PolyDegree::T)?;
        let shares_right = shamir.generate_shares(right, PolyDegree::T)?;
        let shares_result = shamir.generate_shares(&result, PolyDegree::T)?;
        let mut triples = Vec::new();
        for party in 0..=shamir.polynomial_degree() as usize {
            let left_operand = shares_left.get_share(party)?;
            let right_operand = shares_right.get_share(party)?;
            let result = shares_result.get_share(party)?;
            let triple = BeaverTriple::new(left_operand, right_operand, result);
            triples.push(triple);
        }
        Ok(triples)
    }

    fn hide_secrets(
        left: &ModularNumber<Prime>,
        right: &ModularNumber<Prime>,
        shamir: &Shamir<PrimeField<Prime>>,
    ) -> Result<(PointSequence<U64Field>, PointSequence<U64Field>)> {
        let left_shares = shamir.generate_shares(left, PolyDegree::T)?;
        let right_shares = shamir.generate_shares(right, PolyDegree::T)?;
        Ok((left_shares, right_shares))
    }

    #[test]
    fn multiplication() -> Result<()> {
        let shamir = make_shamir();
        let beaver_left = ModularNumber::from_u32(104);
        let beaver_right = ModularNumber::from_u32(31);
        let triples = make_triples(&beaver_left, &beaver_right, &shamir)?;
        // The secrets we want to multiply
        let left_secret = ModularNumber::from_u32(42);
        let right_secret = ModularNumber::from_u32(1337);
        let (left_shares, right_shares) = hide_secrets(&left_secret, &right_secret, &shamir)?;

        // Go through every party, run the multiplication preparation protocol, and save the resulting shares.
        let mut left_scalar_shares = Vec::new();
        let mut right_scalar_shares = Vec::new();
        for party in 0..triples.len() {
            // Get the shares of the triple, left, and right secrets for this party.
            let triple = &triples[party];
            let left_share = left_shares.get_share(party)?;
            let right_share = right_shares.get_share(party)?;
            let (left_share, right_share) = triple.prepare_multiplication(&left_share, &right_share)?;

            let party_id = PartyId::from((party + 1) * 10);
            left_scalar_shares.push((party_id.clone(), left_share));
            right_scalar_shares.push((party_id, right_share));
        }

        // Recover the secret behind the output of the multiplication preparation and ensure it's right.
        let left_scalar = shamir.recover_secret(left_scalar_shares.into_iter())?;
        let right_scalar = shamir.recover_secret(right_scalar_shares.into_iter())?;
        let expected_left = &left_secret - &beaver_left;
        let expected_right = &right_secret - &beaver_right;
        assert_eq!(left_scalar, expected_left);
        assert_eq!(right_scalar, expected_right);

        // Finally, take each of the triples and finalize the multiplication using the recovered scalars.
        let mut final_shares = Vec::new();
        for party in 0..triples.len() {
            let triple = &triples[party];
            let output_share = triple.finalize_multiplication(&left_scalar, &right_scalar)?;
            let party_id = PartyId::from((party + 1) * 10);
            final_shares.push((party_id, output_share));
        }
        let result = shamir.recover_secret(final_shares.into_iter())?;
        assert_eq!(result, &left_secret * &right_secret);
        Ok(())
    }
}
