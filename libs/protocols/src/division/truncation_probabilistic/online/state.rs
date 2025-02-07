//! TRUNCPR protocol.

use crate::reveal::state::{PartySecretMismatch, RevealMode, RevealState, RevealStateMessage};
use anyhow::{anyhow, Error};
use basic_types::PartyMessage;
use math_lib::{
    errors::DivByZero,
    modular::{EncodedModularNumber, ModularInverse, ModularNumber, ModularPow, SafePrime, TryIntoU64},
};
use serde::{Deserialize, Serialize};
use shamir_sharing::{
    party::PartyId,
    secret_sharer::{SafePrimeSecretSharer, ShamirSecretSharer},
};
use state_machine::{
    state::StateMachineMessage, StateMachine, StateMachineOutput, StateMachineState, StateMachineStateExt,
    StateMachineStateOutput, StateMachineStateResult,
};
use state_machine_derive::StateMachineState;
use std::sync::Arc;

type RandomVector<T> = Vec<ModularNumber<T>>;

/// The TRUNCPR protocol state definitions.
pub mod states {
    use crate::reveal::RevealStateMachine;
    use math_lib::{
        fields::PrimeField,
        modular::{ModularNumber, SafePrime},
    };
    use shamir_sharing::secret_sharer::{SafePrimeSecretSharer, ShamirSecretSharer};

    /// The protocol is waiting for masked variable REVEAL.
    pub struct WaitingReveal<T>
    where
        T: SafePrime,
        ShamirSecretSharer<T>: SafePrimeSecretSharer<T>,
    {
        /// The REVEAL state machine.
        pub(crate) reveal_state_machine: RevealStateMachine<PrimeField<T>, ShamirSecretSharer<T>>,

        /// The r % 2^m elements computed.
        pub(crate) random_small: Vec<ModularNumber<T>>,

        /// The shared dividend.
        pub dividends: Vec<ModularNumber<T>>,

        /// The divisors 2^m.
        pub(crate) two_to_m: Vec<ModularNumber<T>>,

        /// The revealed c.
        pub(crate) revealed_variable: Vec<ModularNumber<T>>,
    }
}

/// The input shared dividend and public divisor involved in the probabilistic truncation operation.
#[derive(Clone, Debug)]
pub struct TruncPrShares<T>
where
    T: SafePrime,
{
    /// The shared dividend.
    pub dividend: ModularNumber<T>,

    /// The exponent of the divisors 2^m.
    pub divisors_exp_m: ModularNumber<T>,

    /// The preprocessing elements neeed for this probabilistic truncation.
    pub prep_elements: PrepTruncPrShares<T>,
}

/// The state machine for the trunc pr protocol.
#[derive(StateMachineState)]
#[state_machine(
    recipient_id = "PartyId",
    input_message = "PartyMessage<TruncPrStateMessage>",
    output_message = "TruncPrStateMessage",
    final_result = "Vec<ModularNumber<T>>",
    handle_message_fn = "Self::handle_message"
)]
pub enum TruncPrState<T>
where
    T: SafePrime,
    ShamirSecretSharer<T>: SafePrimeSecretSharer<T>,
{
    /// We are waiting for the c value REVEAL.
    #[state_machine(submachine = "state.reveal_state_machine", transition_fn = "Self::transition_waiting_reveal_c")]
    WaitingReveal(states::WaitingReveal<T>),
}

use TruncPrState::*;

use crate::{
    division::truncation_probabilistic::offline::PrepTruncPrShares, random::random_bitwise::bitwise_shares::merge_bits,
};

impl<T> TruncPrState<T>
where
    T: SafePrime,
    ShamirSecretSharer<T>: SafePrimeSecretSharer<T>,
{
    /// Construct a new TRUNCPR state.
    pub fn new(
        truncpr_elements: Vec<TruncPrShares<T>>,
        secret_sharer: Arc<ShamirSecretSharer<T>>,
        kappa: usize,
        k: usize,
    ) -> Result<(Self, Vec<StateMachineMessage<Self>>), TruncPrCreateError> {
        for truncpr_element in truncpr_elements.iter() {
            let element = truncpr_element
                .divisors_exp_m
                .into_value()
                .try_into_u64()
                .map_err(|_err| TruncPrCreateError::IntegerOverflow)? as usize;
            if element >= k {
                return Err(TruncPrCreateError::DivisorTooLarge);
            }
        }

        // Step 1
        let (random_small, random_full) = Self::build_random_shared_values(&truncpr_elements, k, kappa)?;

        // Step 2
        let b_elements = Self::build_b_share(&truncpr_elements, &random_full, k)?;

        // Step 3
        let mut two_to_m = Vec::new();
        let two = ModularNumber::<T>::two();
        for m in truncpr_elements.iter() {
            two_to_m.push(two.exp_mod(&m.divisors_exp_m.into_value()));
        }

        let dividends = truncpr_elements.iter().map(|mod_element| mod_element.dividend).collect();
        let (reveal_state, messages) = RevealState::new(RevealMode::new_all(b_elements), secret_sharer.clone())?;
        let next_state = states::WaitingReveal {
            reveal_state_machine: StateMachine::new(reveal_state),
            random_small,
            dividends,
            two_to_m,
            revealed_variable: Vec::new(),
        };
        let messages = messages.into_iter().map(|message| message.wrap(&TruncPrStateMessage::Reveal)).collect();
        Ok((WaitingReveal(next_state), messages))
    }

    #[allow(clippy::indexing_slicing)]
    fn transition_waiting_reveal_c(state: states::WaitingReveal<T>) -> StateMachineStateResult<Self> {
        // Step 4
        let c_prime_variable = Self::build_c_prime_variable(state.revealed_variable, &state.two_to_m)
            .map_err(|e| anyhow!("build c' shared values failed: {e}"))?;
        // Step 5
        let a_primes = Self::build_a_prime(c_prime_variable, &state.random_small, &state.two_to_m)
            .map_err(|e| anyhow!("build a' shared values failed: {e}"))?;
        // Step 6
        let output = Self::build_trunc_outputs(&state.dividends, &a_primes, &state.two_to_m)?;
        Ok(StateMachineStateOutput::Final(output))
    }

    fn handle_message(mut state: Self, message: PartyMessage<TruncPrStateMessage>) -> StateMachineStateResult<Self> {
        use TruncPrStateMessage::*;
        let (party_id, message) = message.into_parts();
        match (message, &mut state) {
            (Reveal(message), WaitingReveal(inner)) => {
                match inner.reveal_state_machine.handle_message(PartyMessage::new(party_id, message))? {
                    StateMachineOutput::Final(values) => {
                        inner.revealed_variable = values;
                        state.try_next()
                    }
                    output => state.wrap_message(output, TruncPrStateMessage::Reveal),
                }
            }
        }
    }

    fn build_random_shared_values(
        truncpr_elements: &[TruncPrShares<T>],
        k: usize,
        kappa: usize,
    ) -> Result<(RandomVector<T>, RandomVector<T>), TruncPrCreateError> {
        let mut random_small = Vec::new();
        let mut random_full = Vec::new();
        for truncpr_element in truncpr_elements {
            let element = truncpr_element
                .divisors_exp_m
                .into_value()
                .try_into_u64()
                .map_err(|_err| TruncPrCreateError::IntegerOverflow)? as usize;
            let k_plus_kappa = k.checked_add(kappa).ok_or(TruncPrCreateError::IntegerOverflow)?;
            if truncpr_element.prep_elements.ran_bits_r.shares().len() != k_plus_kappa {
                return Err(TruncPrCreateError::ExpectedSharedRandomBits(
                    k_plus_kappa,
                    truncpr_element.prep_elements.ran_bits_r.shares().len(),
                ));
            }
            let r_small_bits = truncpr_element
                .prep_elements
                .ran_bits_r
                .shares()
                .get(0..element)
                .ok_or(TruncPrCreateError::OutOfBounds)?;
            let r_small = merge_bits(r_small_bits);
            let r_full_bits = &truncpr_element.prep_elements.ran_bits_r;
            let r_full = r_full_bits.merge_bits();
            random_small.push(r_small);
            random_full.push(r_full);
        }
        Ok((random_small, random_full))
    }

    fn build_b_share(
        truncpr_shares: &[TruncPrShares<T>],
        random_full: &[ModularNumber<T>],
        k: usize,
    ) -> Result<Vec<ModularNumber<T>>, TruncPrCreateError> {
        let k_minus_one = k.checked_sub(1).ok_or(TruncPrCreateError::IntegerOverflow)?;
        let two_to_k_minus_one = ModularNumber::two().exp_mod(&T::Normal::from(k_minus_one as u64));

        let mut b_elements = Vec::new();
        for (mod_element, random_value) in truncpr_shares.iter().zip(random_full.iter()) {
            let mut b = two_to_k_minus_one + &mod_element.dividend;
            b = b + random_value;
            b_elements.push(b);
        }
        Ok(b_elements)
    }

    fn build_c_prime_variable(
        c_variable: Vec<ModularNumber<T>>,
        two_to_m: &[ModularNumber<T>],
    ) -> Result<Vec<ModularNumber<T>>, Error> {
        let mut c_prime = Vec::new();
        for (c, two_m) in c_variable.into_iter().zip(two_to_m.iter()) {
            let c_p = (c % two_m)?;
            c_prime.push(c_p);
        }
        Ok(c_prime)
    }

    fn build_a_prime(
        c_primes: Vec<ModularNumber<T>>,
        r_primes: &[ModularNumber<T>],
        two_to_m: &[ModularNumber<T>],
    ) -> Result<Vec<ModularNumber<T>>, Error> {
        let zipped = c_primes.into_iter().zip(r_primes.iter()).zip(two_to_m.iter());
        let mut a_primes = Vec::new();
        for ((c, r), two_m) in zipped {
            let mut a_prime = c - r;
            a_prime = a_prime + two_m;
            a_primes.push(a_prime);
        }
        Ok(a_primes)
    }

    // [d] = ([a] - [a'])(2^{-m} \mod q);
    fn build_trunc_outputs(
        dividends: &[ModularNumber<T>],
        a_primes: &[ModularNumber<T>],
        two_to_m: &[ModularNumber<T>],
    ) -> Result<Vec<ModularNumber<T>>, Error> {
        let zipped = dividends.iter().zip(a_primes.iter()).zip(two_to_m.iter());
        let mut trunc_outputs = Vec::new();
        for ((a, a_prime), two_m) in zipped {
            let mut d = a - a_prime;
            let inv_two_m = two_m.inverse();
            d = d * &inv_two_m;
            trunc_outputs.push(d);
        }
        Ok(trunc_outputs)
    }
}

/// A message for this state machine.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[repr(u8)]
pub enum TruncPrStateMessage {
    /// A message for the REVEAL state machine.
    Reveal(RevealStateMessage<EncodedModularNumber>) = 0,
}

/// An error during the TRUNCPR state construction.
#[derive(thiserror::Error, Debug)]
pub enum TruncPrCreateError {
    /// An error during the REVEAL creation.
    #[error("REVEAL: {0}")]
    Reveal(#[from] PartySecretMismatch),

    /// An error when m > k
    #[error("The size of divisor (m) is larger than the allowed size (k)")]
    DivisorTooLarge,

    /// Integer overflow error.
    #[error("integer overflow")]
    IntegerOverflow,

    /// An error with custom message and parameters
    #[error("Expected {0} shared random bits, got {1}")]
    ExpectedSharedRandomBits(usize, usize),

    /// An arithmetic error.
    #[error("arithmetic: {0}")]
    Arithmetic(#[from] DivByZero),

    /// Value is out of bounds
    #[error("out of bounds")]
    OutOfBounds,
}
