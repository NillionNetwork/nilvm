//! RAN protocol to produce shares from multiple random numbers.

use anyhow::{anyhow, Context};
use basic_types::{jar::PartyJar, PartyMessage};
use math_lib::modular::{EncodedModularNumber, ModularNumber, SafePrime};
use serde::{Deserialize, Serialize};
use shamir_sharing::{
    party::PartyId,
    protocol::PolyDegree,
    secret_sharer::{GenerateSharesError, PartyShares, SecretSharer, SecretSharerProperties},
};
use state_machine::{
    errors::StateMachineError,
    state::{Recipient, StateMachineMessage},
    StateMachineState, StateMachineStateExt, StateMachineStateOutput, StateMachineStateResult,
};
use state_machine_derive::StateMachineState;
use std::{collections::HashMap, sync::Arc};

/// The content of the states for the RAN state.
pub mod states {
    use basic_types::jar::PartyJar;
    use math_lib::modular::{ModularNumber, SafePrime};
    use shamir_sharing::secret_sharer::{SafePrimeSecretSharer, ShamirSecretSharer};
    use std::sync::Arc;

    /// We are waiting for multiple shares of each of the parties' random number.
    pub struct WaitingShares<T>
    where
        T: SafePrime,
        ShamirSecretSharer<T>: SafePrimeSecretSharer<T>,
    {
        /// The number of elements expected out of this protocol run.
        pub(crate) element_count: usize,

        /// The number of runs expected out of this protocol run.
        pub(crate) run_count: usize,

        /// The random number shares for each of the participants, indexed by their party identifier.
        pub(crate) party_shares: PartyJar<Vec<ModularNumber<T>>>,

        /// The secret sharer to be used.
        pub(crate) secret_sharer: Arc<ShamirSecretSharer<T>>,
    }
}

/// The state for a RAN protocol. This allows running N individual RANs within the same state machine.
#[derive(StateMachineState)]
#[state_machine(
    recipient_id = "PartyId",
    input_message = "PartyMessage<RandomIntegerStateMessage>",
    output_message = "RandomIntegerStateMessage",
    final_result = "Vec<ModularNumber<T>>",
    handle_message_fn = "Self::handle_message"
)]
pub enum RandomIntegerState<T>
where
    T: SafePrime,
    ShamirSecretSharer<T>: SafePrimeSecretSharer<T>,
{
    /// We are waiting for shares.
    #[state_machine(completed = "state.party_shares.is_full()", transition_fn = "Self::transition_waiting_shares")]
    WaitingShares(states::WaitingShares<T>),
}

use shamir_sharing::secret_sharer::{SafePrimeSecretSharer, ShamirSecretSharer};
use RandomIntegerState::*;

/// The mode we want to run RAN on.
///
/// The whitepaper runs 2 flavors of RAN:
/// * RAN where random degree T sharings of *RANDOM* elements are generated: [r]_T <- RAN()
/// * RAN where random degree 2T sharings of *ZERO* elements are generated: [0]_2T <- RAN(MODE::ZERO)
///
/// This enum wraps that behavior: [`RandomMode::RandomOfDegreeT`] is the first one, [`RandomMode::ZerosOfDegree2T`] is the second one.
#[derive(Debug, Clone)]
pub enum RandomMode {
    /// We are picking random numbers to share with a degree T polynomial.
    RandomOfDegreeT,

    /// We are sharing 0s with a degree 2T polynomial.
    ZerosOfDegree2T,
}

impl<T> RandomIntegerState<T>
where
    T: SafePrime,
    ShamirSecretSharer<T>: SafePrimeSecretSharer<T>,
{
    /// Constructs a new RAN state, returning the instance plus the initial set
    /// of messages that this state machine needs to exchange with other nodes.
    pub fn new(
        mode: RandomMode,
        element_count: usize,
        secret_sharer: Arc<ShamirSecretSharer<T>>,
    ) -> Result<(Self, Vec<StateMachineMessage<Self>>), RandomIntegerError> {
        // The protocol generates m = n-t random values per run. To create c = element_count random
        // values, it needs to run r = (c / (n-t)) times. To make sure we get at least c elements,
        // we do a ceiling division by (c + m - 1) / m instead of floor division c / m.
        // m = n - t
        let m = secret_sharer
            .party_count()
            .checked_sub(secret_sharer.polynomial_degree() as usize)
            .ok_or(RandomIntegerError::IntegerOverflow)?;
        // r = (c + m - 1)
        let run_count = element_count
            .checked_add(m)
            .ok_or(RandomIntegerError::IntegerOverflow)?
            .checked_sub(1)
            .ok_or(RandomIntegerError::IntegerOverflow)?;
        // r = (c + m - 1) / m
        let run_count = run_count.checked_div(m).ok_or(RandomIntegerError::IntegerOverflow)?;
        let mut random_numbers = Vec::new();
        for _ in 0..run_count {
            let element = match mode {
                RandomMode::RandomOfDegreeT => ModularNumber::<T>::gen_random(),
                RandomMode::ZerosOfDegree2T => ModularNumber::ZERO,
            };
            random_numbers.push(element);
        }
        let messages = Self::make_messages(random_numbers, mode, &secret_sharer)?;
        let party_shares = PartyJar::new(secret_sharer.party_count());
        let state = states::WaitingShares { element_count, run_count, party_shares, secret_sharer };
        Ok((WaitingShares(state), messages))
    }

    fn make_messages(
        random_numbers: Vec<ModularNumber<T>>,
        mode: RandomMode,
        secret_sharer: &ShamirSecretSharer<T>,
    ) -> Result<Vec<StateMachineMessage<Self>>, RandomIntegerError> {
        let degree = match mode {
            RandomMode::RandomOfDegreeT => PolyDegree::T,
            RandomMode::ZerosOfDegree2T => PolyDegree::TwoT,
        };
        let party_shares: PartyShares<Vec<ModularNumber<T>>> =
            secret_sharer.generate_shares(&random_numbers, degree)?;
        let mut messages = Vec::new();
        for (party_id, shares) in party_shares {
            let contents = RandomIntegerStateMessage(shares.into_iter().map(|s| s.encode()).collect());
            messages.push(StateMachineMessage::<Self>::new(Recipient::Single(party_id), contents));
        }
        Ok(messages)
    }

    // Transform the structure of N shares per party into a vec of vecs where the inner vec contains the share from
    // each party.
    #[allow(clippy::indexing_slicing)]
    fn transition_waiting_shares(state: states::WaitingShares<T>) -> StateMachineStateResult<Self> {
        let mut random_shares = vec![HashMap::new(); state.run_count];
        for (party_id, party_shares) in state.party_shares.into_elements() {
            // Note: we need these checks because someone could have misused the state machine.
            if party_shares.len() != state.run_count {
                return Err(StateMachineError::UnexpectedError(anyhow!(
                    "not enough shares: provided {}, needed {}",
                    party_shares.len(),
                    state.run_count
                )));
            }
            for (index, share) in party_shares.into_iter().enumerate() {
                random_shares[index].insert(party_id.clone(), share);
            }
        }
        // Now just individually map each inner vec to get our shares of the output random number.
        let mut output_shares = Vec::new();
        for shares in random_shares {
            let result_shares =
                state.secret_sharer.hyper_map(shares.into_iter()).map_err(|e| anyhow!("failed to map shares: {e}"))?;
            for share in result_shares.into_iter() {
                output_shares.push(share);
            }
        }
        if output_shares.len() > state.element_count {
            output_shares = output_shares.into_iter().take(state.element_count).collect();
        }
        Ok(StateMachineStateOutput::Final(output_shares))
    }

    fn handle_message(
        mut state: Self,
        message: <Self as StateMachineState>::InputMessage,
    ) -> StateMachineStateResult<Self> {
        let (party_id, message) = message.into_parts();
        let shares =
            message.0.into_iter().map(|m| m.try_decode()).collect::<Result<_, _>>().context("decoding shares")?;
        state.waiting_shares_state_mut()?.party_shares.add_element(party_id, shares).context("adding shares")?;
        state.advance_if_completed()
    }
}

/// A message for this state machine, which sets the shares from a particular party.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct RandomIntegerStateMessage(pub Vec<EncodedModularNumber>);

/// An error during the STANDARD-RAN state construction.
#[derive(thiserror::Error, Debug)]
pub enum RandomIntegerError {
    /// Share generation failed.
    #[error(transparent)]
    GenerateShares(#[from] GenerateSharesError),

    /// A party id was not found.
    #[error("party id not found")]
    PartyNotFound,

    /// Integer arithmetic overflow.
    #[error("integer overflow")]
    IntegerOverflow,
}

#[allow(clippy::integer_arithmetic, clippy::indexing_slicing)]
#[cfg(test)]
mod test {
    use super::*;
    use anyhow::Result;
    use math_lib::modular::{ModularNumber, U64SafePrime};
    use shamir_sharing::secret_sharer::{SecretSharer, ShamirSecretSharer};
    use state_machine::StateMachine;
    // use math_lib::modular::U64SafePrime;

    type Prime = U64SafePrime;
    type Sharer = ShamirSecretSharer<Prime>;
    type State = RandomIntegerState<U64SafePrime>;

    fn make_secret_sharer(parties: usize) -> Arc<Sharer> {
        let degree = (parties - 1) as u64;
        let parties: Vec<_> = (1..=parties).map(|id| PartyId::from(id * 10)).collect();
        let sharer = Sharer::new(parties[0].clone(), degree, parties).unwrap();
        Arc::new(sharer)
    }

    #[test]
    fn random_share_message_creation() -> Result<()> {
        // Generate messages for 2 random numbers, recover them, and ensure we get them back.
        let secret_sharer = make_secret_sharer(2);
        let random_numbers = vec![ModularNumber::from_u32(42), ModularNumber::from_u32(1337)];
        let messages = State::make_messages(random_numbers.clone(), RandomMode::RandomOfDegreeT, &secret_sharer)?;

        // Collect them back
        let mut party_shares: PartyShares<Vec<ModularNumber<Prime>>> = PartyShares::default();
        for message in messages {
            let party_id = match message.recipient() {
                Recipient::Single(party_id) => party_id.clone(),
                _ => return Err(anyhow!("not a single recipient")),
            };
            match message.into_contents() {
                RandomIntegerStateMessage(shares) => {
                    assert_eq!(shares.len(), 2);
                    let decoded_shares = shares.into_iter().map(|s| s.try_decode()).collect::<Result<Vec<_>, _>>()?;
                    party_shares.insert(party_id, decoded_shares);
                }
            }
        }
        let recovered_secrets = secret_sharer.recover(party_shares).unwrap();
        assert_eq!(recovered_secrets, random_numbers);
        Ok(())
    }

    #[test]
    fn not_enough_shares() -> Result<()> {
        let secret_sharer = make_secret_sharer(2);
        // 2 shares, 1 party
        let mut state = State::new(RandomMode::RandomOfDegreeT, 2, secret_sharer)?.0;
        state
            .waiting_shares_state_mut()?
            .party_shares
            .add_element(PartyId::from(1), vec![ModularNumber::from_u32(10)])
            .unwrap();
        assert!(state.try_next().is_err());
        Ok(())
    }

    #[test]
    fn too_many_shares() -> Result<()> {
        let secret_sharer = make_secret_sharer(2);
        // 1 shares, 1 party
        let mut state = State::new(RandomMode::RandomOfDegreeT, 1, secret_sharer)?.0;
        state
            .waiting_shares_state_mut()?
            .party_shares
            .add_element(PartyId::from(1), vec![ModularNumber::from_u32(10), ModularNumber::from_u32(20)])
            .unwrap();
        assert!(state.try_next().is_err());
        Ok(())
    }

    #[test]
    fn waiting_multi_checks() -> Result<()> {
        let secret_sharer = make_secret_sharer(2);
        // 3 shares, 2 parties
        let mut sm = StateMachine::new(State::new(RandomMode::RandomOfDegreeT, 3, secret_sharer)?.0);
        assert!(!sm.is_state_completed());
        assert!(!sm.is_finished());

        // Push one party's numbers, we shouldn't be done yet
        sm.handle_message(PartyMessage::new(
            PartyId::from(10),
            RandomIntegerStateMessage(vec![
                ModularNumber::<U64SafePrime>::from_u32(10).encode(),
                ModularNumber::<U64SafePrime>::from_u32(20).encode(),
                ModularNumber::<U64SafePrime>::from_u32(30).encode(),
            ]),
        ))?;
        assert!(!sm.is_state_completed());

        // Now push the other's, we should be done
        let shares = sm
            .handle_message(PartyMessage::new(
                PartyId::from(20),
                RandomIntegerStateMessage(vec![
                    ModularNumber::<U64SafePrime>::from_u32(41).encode(),
                    ModularNumber::<U64SafePrime>::from_u32(42).encode(),
                    ModularNumber::<U64SafePrime>::from_u32(43).encode(),
                ]),
            ))?
            .into_final()?;

        let expected = vec![ModularNumber::from_u32(51), ModularNumber::from_u32(62), ModularNumber::from_u32(73)];
        assert_eq!(shares, expected);

        Ok(())
    }
}
