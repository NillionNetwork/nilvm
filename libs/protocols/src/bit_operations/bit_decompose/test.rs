#![allow(clippy::arithmetic_side_effects, clippy::panic, clippy::indexing_slicing)]

use crate::{
    bit_operations::bit_decompose::state::{BitDecomposeOperands, BitDecomposeState},
    random::random_bitwise::BitwiseNumberShares,
    simulator::symmetric::{InitializedProtocol, Protocol, SymmetricProtocolSimulator},
};
use anyhow::{anyhow, Error, Result};
use math_lib::{
    fields::PrimeField,
    modular::{AsBits, Modular, ModularNumber, SafePrime, U128SafePrime, U256SafePrime, U64SafePrime},
    polynomial::{point::Point, point_sequence::PointSequence},
};
use rstest::rstest;
use shamir_sharing::{
    party::{PartyId, PartyMapper},
    protocol::{PolyDegree, Shamir},
    secret_sharer::{FieldSecretSharer, PartyShares, ShamirSecretSharer},
};
use std::{collections::HashMap, marker::PhantomData, sync::Arc};

struct BitDecomposeProtocol<T: SafePrime> {
    inputs: Vec<u64>,
    randoms: Vec<ModularNumber<T>>,
    polynomial_degree: u64,
    _unused: PhantomData<T>,
}

impl<T: SafePrime> BitDecomposeProtocol<T> {
    fn new(inputs: Vec<u64>, randoms: Vec<ModularNumber<T>>, polynomial_degree: u64) -> Self {
        Self { inputs, randoms, polynomial_degree, _unused: Default::default() }
    }

    fn validate_output(self, party_shares_outputs: HashMap<PartyId, Vec<BitwiseNumberShares<T>>>) -> Result<()> {
        // Reconstruct the outputs.
        let mapper = PartyMapper::<PrimeField<T>>::new(party_shares_outputs.keys().cloned().collect())?;
        let mut point_sequences =
            vec![vec![PointSequence::<PrimeField<T>>::default(); T::MODULO.bits()]; self.inputs.len()];
        for (party_id, party_shares) in party_shares_outputs {
            if party_shares.len() != self.inputs.len() {
                return Err(anyhow!(
                    "unexpected element share count: expected {}, got {}",
                    self.inputs.len(),
                    party_shares.len()
                ));
            }
            let x =
                *mapper.abscissa(&party_id).ok_or_else(|| anyhow!("failed to find abscissa for party {party_id:?}"))?;
            for (element_index, shares) in party_shares.into_iter().enumerate() {
                for (bit_index, share) in shares.shares().into_iter().enumerate() {
                    point_sequences[element_index][bit_index].push(Point::new(x, *share.value()));
                }
            }
        }

        // Lagrange interpolate the outputs and check against the expected result.
        let zipped = point_sequences.into_iter().zip(self.inputs.iter());
        for (point_sequences, input) in zipped {
            let mut output = ModularNumber::ZERO;
            let mut factor = ModularNumber::ONE;
            for sequence in point_sequences {
                let bit = sequence.lagrange_interpolate()?;
                output = output + &(factor * &bit);
                factor = factor * &ModularNumber::two();
            }

            let expected_value = ModularNumber::from_u64(*input);
            assert_eq!(output, expected_value, "failed for {}", input);
        }

        Ok(())
    }
}

impl<T: SafePrime> Protocol for BitDecomposeProtocol<T>
where
    T: SafePrime,
    ShamirSecretSharer<T>: FieldSecretSharer<PrimeField<T>> + FieldSecretSharer<PrimeField<T::SophiePrime>>,
{
    type State = BitDecomposeState<T>;
    type PrepareOutput = BitDecomposeConfig<T>;

    fn prepare(&self, parties: &[PartyId]) -> Result<Self::PrepareOutput, Error> {
        let parties = parties.to_vec();
        let mapper = PartyMapper::<PrimeField<T>>::new(parties.clone())?;
        // Note: the party id doesn't matter in this context
        let shamir = Shamir::<PrimeField<T>>::new(PartyId::from(0), self.polynomial_degree, parties.clone())?;

        let mut party_operands: PartyShares<Vec<BitDecomposeOperands<T>>> = PartyShares::default();
        for (input, random) in self.inputs.iter().zip(self.randoms.iter()) {
            let shares = shamir.generate_shares(&ModularNumber::from_u64(*input), PolyDegree::T)?;
            let mut party_bit_shares: PartyShares<Vec<ModularNumber<T>>> = PartyShares::default();
            for i in 0..T::MODULO.bits() {
                let bit = ModularNumber::from_u64(random.into_value().bit(i) as u64);
                let bit_shares = shamir.generate_shares(&bit, PolyDegree::T)?;
                for bit_share in bit_shares.into_points().into_iter() {
                    let (x, y) = bit_share.into_coordinates();
                    let party_id = mapper.party(&x).ok_or_else(|| anyhow!("party id for {x:?} not found"))?;
                    party_bit_shares.entry(party_id.clone()).or_default().push(y);
                }
            }
            for share in shares.into_points().into_iter() {
                let (x, y) = share.into_coordinates();
                let party_id = mapper.party(&x).ok_or_else(|| anyhow!("party id for {x:?} not found"))?;
                let bitwise = party_bit_shares.get(&party_id).unwrap();
                let operand = BitDecomposeOperands::new(y, bitwise.clone().into());
                party_operands.entry(party_id.clone()).or_default().push(operand);
            }
        }
        Ok(BitDecomposeConfig { parties: parties.to_vec(), party_operands })
    }

    fn initialize(
        &self,
        party_id: PartyId,
        config: &Self::PrepareOutput,
    ) -> Result<InitializedProtocol<Self::State>, anyhow::Error> {
        let operands =
            config.party_operands.get(&party_id).cloned().ok_or_else(|| anyhow!("shares for party {party_id:?}"))?;
        let secret_sharer = ShamirSecretSharer::new(party_id, self.polynomial_degree, config.parties.clone())?;
        let (state, messages) = BitDecomposeState::new(operands, Arc::new(secret_sharer))?;

        Ok(InitializedProtocol::new(state, messages))
    }
}

/// The internal configuration of a BIT-DECOMPOSE protocol.
struct BitDecomposeConfig<T: Modular> {
    parties: Vec<PartyId>,
    party_operands: PartyShares<Vec<BitDecomposeOperands<T>>>,
}

#[rstest]
#[case::u64(U64SafePrime)]
#[case::u128(U128SafePrime)]
#[case::u256(U256SafePrime)]
fn end_to_end<T: SafePrime>(#[case] _prime: T)
where
    ShamirSecretSharer<T>: FieldSecretSharer<PrimeField<T>> + FieldSecretSharer<PrimeField<T::SophiePrime>>,
{
    let max_rounds = 100;
    let polynomial_degree = 1;
    let network_size = 3;

    let inputs = vec![16, 4, 6, 34, 66, 135, 1464, 413, 9766, 1344, 1326];
    let mut rng = rand::thread_rng();
    let randoms = inputs.iter().map(|_| ModularNumber::gen_random_with_rng(&mut rng)).collect();
    let protocol = BitDecomposeProtocol::<T>::new(inputs, randoms, polynomial_degree);
    let simulator = SymmetricProtocolSimulator::new(network_size, max_rounds);
    let outputs = simulator.run_protocol(&protocol).expect("protocol run failed");
    let mut party_shares = HashMap::new();
    for output in outputs {
        match output.output {
            shares => {
                party_shares.insert(output.party_id, shares);
            }
        };
    }

    protocol.validate_output(party_shares).expect("validation failed");
}
