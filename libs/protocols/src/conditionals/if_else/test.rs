#![allow(clippy::arithmetic_side_effects, clippy::panic, clippy::indexing_slicing)]

use super::state::{IfElseOperands, IfElseState};
use crate::simulator::symmetric::{InitializedProtocol, Protocol, SymmetricProtocolSimulator};
use anyhow::{anyhow, Error, Result};
use math_lib::{
    fields::PrimeField,
    modular::{ModularNumber, SafePrime, U256SafePrime},
    polynomial::{point::Point, point_sequence::PointSequence},
};
use shamir_sharing::{
    party::{PartyId, PartyMapper},
    secret_sharer::{FieldSecretSharer, ShamirSecretSharer},
};
use std::{collections::HashMap, marker::PhantomData, sync::Arc};

struct IfElseProtocol<T: SafePrime> {
    if_else_input_operands: Vec<IfElseOperands<T>>,
    polynomial_degree: u64,
    _unused: PhantomData<T>,
}

impl<T: SafePrime> IfElseProtocol<T> {
    fn new(if_else_operands: Vec<IfElseOperands<T>>, polynomial_degree: u64) -> Self {
        Self { if_else_input_operands: if_else_operands, polynomial_degree, _unused: Default::default() }
    }

    // cond, left, right
    fn validate_output(self, party_shares_outputs: HashMap<PartyId, Vec<ModularNumber<T>>>) -> Result<()> {
        // Reconstruct the outputs.
        let mapper = PartyMapper::<PrimeField<T>>::new(party_shares_outputs.keys().cloned().collect())?;
        let mut point_sequences = vec![PointSequence::<PrimeField<T>>::default(); self.if_else_input_operands.len()];
        for (party_id, party_shares) in party_shares_outputs {
            if party_shares.len() != self.if_else_input_operands.len() {
                return Err(anyhow!(
                    "unexpected element share count: expected {}, got {}",
                    self.if_else_input_operands.len(),
                    party_shares.len()
                ));
            }
            let x =
                *mapper.abscissa(&party_id).ok_or_else(|| anyhow!("failed to find abscissa for party {party_id:?}"))?;
            for (element_index, share) in party_shares.into_iter().enumerate() {
                point_sequences[element_index].push(Point::new(x, share));
            }
        }

        // Lagrange interpolate the outputs and check against the expected result.
        let zipped = point_sequences.into_iter().zip(self.if_else_input_operands.iter());
        for (point_sequence, if_else_op) in zipped {
            let comparison_output = point_sequence.lagrange_interpolate()?;

            // The actual check is "if cond { left } else { right }".
            let expected_value = {
                // Prevent formatting.
                if if_else_op.cond.into_value() == 1.into() { if_else_op.left } else { if_else_op.right }
            };
            assert_eq!(
                comparison_output,
                expected_value,
                "failed for {} vs {}",
                if_else_op.left.into_value(),
                if_else_op.right.into_value()
            );
        }

        Ok(())
    }
}

impl<T: SafePrime> Protocol for IfElseProtocol<T>
where
    T: SafePrime,
    ShamirSecretSharer<T>: FieldSecretSharer<PrimeField<T>> + FieldSecretSharer<PrimeField<T::SophiePrime>>,
{
    type State = IfElseState<T>;
    type PrepareOutput = IfElseConfig;

    fn prepare(&self, parties: &[PartyId]) -> Result<Self::PrepareOutput, Error> {
        Ok(IfElseConfig { parties: parties.to_vec() })
    }

    fn initialize(
        &self,
        party_id: PartyId,
        config: &Self::PrepareOutput,
    ) -> Result<InitializedProtocol<Self::State>, anyhow::Error> {
        let secret_sharer = ShamirSecretSharer::new(party_id, self.polynomial_degree, config.parties.clone())?;
        let (state, messages) = IfElseState::new(self.if_else_input_operands.clone(), Arc::new(secret_sharer))?;

        Ok(InitializedProtocol::new(state, messages))
    }
}

struct IfElseConfig {
    parties: Vec<PartyId>,
}

#[test]
fn end_to_end() {
    let max_rounds = 100;
    let polynomial_degree = 1;
    let network_size = 5;

    let inputs = vec![
        // if false { 2 } else { 13 }
        IfElseOperands::new(ModularNumber::ZERO, ModularNumber::two(), ModularNumber::from_u32(13)),
        // if true { 7 } else { 5 }
        IfElseOperands::new(ModularNumber::ONE, ModularNumber::from_u32(7), ModularNumber::from_u32(5)),
        // if false { 7 } else { 5 }
        IfElseOperands::new(ModularNumber::ZERO, ModularNumber::from_u32(7), ModularNumber::from_u32(5)),
    ];

    let protocol = IfElseProtocol::<U256SafePrime>::new(inputs, polynomial_degree);
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
