//! Share multiplication and reveal protocol.

use anyhow::{anyhow, Context};
use basic_types::{jar::PartyJar, PartyMessage};
use math_lib::{
    errors::DivByZero,
    fields::{Field, PrimeField},
    modular::{EncodedModularNumber, Modular, ModularNumber, SafePrime},
};
use serde::{Deserialize, Serialize};
use shamir_sharing::{
    party::PartyId,
    secret_sharer::{
        GenerateSharesError, SafePrimeSecretSharer, SecretSharer, SecretSharerProperties, ShamirSecretSharer,
    },
};
use state_machine::{
    errors::StateMachineError,
    state::{Recipient, StateMachineMessage},
    StateMachineStateExt, StateMachineStateOutput, StateMachineStateResult,
};
use state_machine_derive::StateMachineState;
use std::{collections::HashMap, sync::Arc};

/// The public multiplication protocol state definitions.
pub mod states {
    use basic_types::jar::PartyJar;
    use math_lib::modular::{ModularNumber, SafePrime};
    use shamir_sharing::secret_sharer::ShamirSecretSharer;
    use std::sync::Arc;

    /// The protocol is waiting for each parties' share of the local product.
    pub struct WaitingShares<T: SafePrime> {
        /// The expected number of shares.
        pub share_count: usize,

        /// The secret sharer this protocol is using.
        pub secret_sharer: Arc<ShamirSecretSharer<T>>,

        /// The shares of the product from each party, indexed by their party id.
        pub party_shares: PartyJar<Vec<ModularNumber<T>>>,
    }
}

/// The shares of the operands involved in the multiplication.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PubOperandShares<T: Modular> {
    /// The share of the left operand.
    pub left: Vec<ModularNumber<T>>,

    /// The share of the right operand.
    pub right: Vec<ModularNumber<T>>,

    /// The degree 2T zero share.
    pub zero: ModularNumber<T>,
}

impl<T: Modular> PubOperandShares<T> {
    /// Constructs a new operand shares.
    pub fn new(left: Vec<ModularNumber<T>>, right: Vec<ModularNumber<T>>, zero: ModularNumber<T>) -> Self {
        Self { left, right, zero }
    }

    /// Constructs a new operand that has single shares.
    pub fn single(left: ModularNumber<T>, right: ModularNumber<T>, zero: ModularNumber<T>) -> Self {
        Self { left: vec![left], right: vec![right], zero }
    }
}

/// The state machine for the public multiplication protocol.
#[derive(StateMachineState)]
#[state_machine(
    recipient_id = "PartyId",
    input_message = "PartyMessage<PubMultStateMessage>",
    output_message = "PubMultStateMessage",
    final_result = "Vec<ModularNumber<T>>",
    handle_message_fn = "Self::handle_message"
)]
pub enum PubMultState<T>
where
    T: SafePrime,
    ShamirSecretSharer<T>: SafePrimeSecretSharer<T>,
{
    /// We are waiting for the shares from each party.
    #[state_machine(completed = "state.party_shares.is_full()", transition_fn = "Self::transition_waiting_shares")]
    WaitingShares(states::WaitingShares<T>),
}

use PubMultState::*;

impl<T> PubMultState<T>
where
    T: SafePrime,
    ShamirSecretSharer<T>: SafePrimeSecretSharer<T>,
{
    /// Construct a new public multiplication protocol state.
    pub fn new(
        operand_shares: Vec<PubOperandShares<T>>,
        secret_sharer: Arc<ShamirSecretSharer<T>>,
    ) -> Result<(Self, Vec<StateMachineMessage<Self>>), PubMultCreateError> {
        let share_count = operand_shares.len();
        let messages = Self::make_messages(operand_shares, &secret_sharer)?;
        let party_shares = PartyJar::new(secret_sharer.party_count());
        let state = WaitingShares(states::WaitingShares { share_count, secret_sharer, party_shares });
        Ok((state, messages))
    }

    fn make_messages(
        operand_shares: Vec<PubOperandShares<T>>,
        secret_sharer: &ShamirSecretSharer<T>,
    ) -> Result<Vec<StateMachineMessage<Self>>, PubMultCreateError> {
        let mut products = Vec::new();
        for shares in operand_shares {
            if shares.left.len() != shares.right.len() {
                return Err(PubMultCreateError::UnequalLengthOperands(shares.left.len(), shares.right.len()));
            }
            let mut product = shares.zero;
            for (left, right) in shares.left.iter().zip(shares.right.iter()) {
                product = product + &(*left * right);
            }
            products.push(product);
        }
        // Every node gets all shares.
        let parties = secret_sharer.parties();
        let shares = PrimeField::encode(&products);
        let messages =
            vec![StateMachineMessage::<Self>::new(Recipient::Multiple(parties), PubMultStateMessage(shares))];
        Ok(messages)
    }

    fn handle_message(mut state: Self, message: PartyMessage<PubMultStateMessage>) -> StateMachineStateResult<Self> {
        let (party_id, message) = message.into_parts();
        let shares = PrimeField::try_decode(&message.0).context("share decoding")?;
        state.waiting_shares_state_mut()?.party_shares.add_element(party_id, shares).context("adding shares")?;
        state.advance_if_completed()
    }

    #[allow(clippy::indexing_slicing)]
    fn transition_waiting_shares(state: states::WaitingShares<T>) -> StateMachineStateResult<Self> {
        let mut product_shares = vec![HashMap::new(); state.share_count];
        for (party_id, shares) in state.party_shares.into_elements() {
            if shares.len() != state.share_count {
                return Err(StateMachineError::UnexpectedError(anyhow!(
                    "expected {} shares, got {}",
                    state.share_count,
                    shares.len()
                )));
            }
            for (index, share) in shares.into_iter().enumerate() {
                product_shares[index].insert(party_id.clone(), share);
            }
        }
        let mut results = Vec::new();
        for shares in product_shares {
            let result_shares: ModularNumber<T> = state
                .secret_sharer
                .recover(shares.into_iter())
                .map_err(|e| anyhow!("failed to recover shares: {e}"))?;
            results.push(result_shares);
        }
        Ok(StateMachineStateOutput::Final(results))
    }
}

/// A message for the PUB-MULT state machine, which sets the shares from a party.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct PubMultStateMessage(Vec<EncodedModularNumber>);

/// An error during the PUB-MULT state construction.
#[derive(thiserror::Error, Debug)]
pub enum PubMultCreateError {
    /// Multiplying shares failed.
    #[error("share multiplication error: {0}")]
    Operation(#[from] DivByZero),

    /// Share generation failed.
    #[error(transparent)]
    GenerateShares(#[from] GenerateSharesError),

    /// A party id was not found.
    #[error("party id not found")]
    PartyNotFound,

    /// Length of the operands do not match.
    #[error("left.len()={0} is not equal to right.len()={1}")]
    UnequalLengthOperands(usize, usize),
}

#[allow(clippy::arithmetic_side_effects, clippy::indexing_slicing)]
#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::Result;
    use math_lib::modular::U64SafePrime;
    use state_machine::StateMachine;

    type Prime = U64SafePrime;
    type U64Field = PrimeField<Prime>;
    type Sharer = ShamirSecretSharer<Prime>;
    type State = PubMultState<Prime>;

    fn make_secret_sharer() -> Arc<Sharer> {
        let parties = vec![PartyId::from(1), PartyId::from(2), PartyId::from(3)];
        let secret_sharer = Sharer::new(parties[0].clone(), 1, parties).unwrap();
        Arc::new(secret_sharer)
    }

    #[test]
    fn waiting_shares_checks() -> Result<()> {
        let secret_sharer = make_secret_sharer();

        let operands = vec![
            PubOperandShares::single(ModularNumber::from_u32(42), ModularNumber::from_u32(13), ModularNumber::ONE),
            PubOperandShares::single(ModularNumber::from_u32(7), ModularNumber::from_u32(5), ModularNumber::two()),
        ];
        let mut sm = StateMachine::new(State::new(operands, secret_sharer)?.0);
        assert!(!sm.is_state_completed());
        assert!(!sm.is_finished());

        sm.handle_message(PartyMessage::new(
            PartyId::from(1),
            PubMultStateMessage(U64Field::encode(&[ModularNumber::from_u32(100), ModularNumber::from_u32(101)])),
        ))?;
        sm.handle_message(PartyMessage::new(
            PartyId::from(2),
            PubMultStateMessage(U64Field::encode(&[ModularNumber::from_u32(150), ModularNumber::from_u32(151)])),
        ))?;

        // This last one should consume the state machine.
        let results = sm
            .handle_message(PartyMessage::new(
                PartyId::from(3),
                PubMultStateMessage(U64Field::encode(&[ModularNumber::from_u32(200), ModularNumber::from_u32(201)])),
            ))?
            .into_final()?;

        // (3 * 100) + (-3 * 150) + (1 * 200)
        assert_eq!(results, vec![ModularNumber::from_u32(50), ModularNumber::from_u32(51)]);

        Ok(())
    }
}
