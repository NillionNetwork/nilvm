//! Integer division by secret divisor protocol (DIV-INT-SECRET).

use super::utils::*;
use crate::{
    bit_operations::scale::{ScaleState, ScaleStateMessage},
    conditionals::less_than::online::state::{Comparands, CompareCreateError, CompareState, CompareStateMessage},
    division::modulo2m_public_divisor::online::state::{
        states::Mod2mTruncVariant, Modulo2mShares, Modulo2mState, Modulo2mStateMessage,
    },
    multiplication::{
        multiplication_and_truncation::{
            state::{MultTruncCreateError, MultTruncState, MultTruncStateMessage},
            MultTruncShares,
        },
        multiplication_shares::{
            state::{MultState, MultStateMessage},
            OperandShares,
        },
    },
};
use anyhow::anyhow;
use basic_types::PartyMessage;
use math_lib::{
    errors::DivByZero,
    modular::{ModularNumber, SafePrime},
};
use serde::{Deserialize, Serialize};
use shamir_sharing::{
    party::PartyId,
    secret_sharer::{SafePrimeSecretSharer, ShamirSecretSharer},
};
use state_machine::{
    errors::StateMachineError, state::StateMachineMessage, StateMachine, StateMachineOutput, StateMachineState,
    StateMachineStateExt, StateMachineStateOutput, StateMachineStateResult,
};
use state_machine_derive::StateMachineState;
use std::{f64::consts::SQRT_2, sync::Arc};

/// ALPHA parameter used when calculating initial guess
const ALPHA: f64 = 1.5 - SQRT_2;

/// The division protocol state definitions.
pub mod states {
    use crate::{
        bit_operations::scale::ScaleStateMachine,
        conditionals::less_than::CompareStateMachine,
        division::{
            division_secret_divisor::offline::PrepDivisionIntegerSecretShares,
            modulo2m_public_divisor::Modulo2mStateMachine,
        },
        multiplication::{
            multiplication_and_truncation::MultTruncStateMachine, multiplication_shares::MultStateMachine,
        },
    };
    use math_lib::modular::{ModularNumber, SafePrime};
    use shamir_sharing::secret_sharer::{SafePrimeSecretSharer, ShamirSecretSharer};
    use std::sync::Arc;

    /// The protocol waits for the sign calculation.
    ///
    /// Involves performing a share comparison using COMPARE.
    pub struct WaitingSignCalculation<T>
    where
        T: SafePrime,
        ShamirSecretSharer<T>: SafePrimeSecretSharer<T>,
    {
        /// The compare state machine
        pub(crate) state_machine: CompareStateMachine<T>,

        /// The secret sharer
        pub(crate) secret_sharer: Arc<ShamirSecretSharer<T>>,

        /// Security parameter
        pub(crate) security_parameter: usize,

        /// Integer size
        pub(crate) integer_size: usize,

        /// The dividends
        pub(crate) dividends: Vec<ModularNumber<T>>,

        /// The divisors
        pub(crate) divisors: Vec<ModularNumber<T>>,

        /// Prep compare shares
        pub(crate) prep_elements: Vec<PrepDivisionIntegerSecretShares<T>>,

        /// The resulting divisor signs
        pub(crate) divisor_signs: Vec<ModularNumber<T>>,

        /// The resulting dividend signs
        pub(crate) dividend_signs: Vec<ModularNumber<T>>,
    }

    /// Waiting for sign correction operation
    /// Performs share multiplication using MULT (share multiplication protocol) state machine
    /// in order to calculate a sign correction.
    pub struct WaitingSignCorrection<T>
    where
        T: SafePrime,
        ShamirSecretSharer<T>: SafePrimeSecretSharer<T>,
    {
        /// The mult state machine
        pub(crate) state_machine: MultStateMachine<T>,

        /// The secret sharer
        pub(crate) secret_sharer: Arc<ShamirSecretSharer<T>>,

        /// Security parameter
        pub(crate) security_parameter: usize,

        /// Integer size
        pub(crate) integer_size: usize,

        /// Prep compare shares
        pub(crate) prep_elements: Vec<PrepDivisionIntegerSecretShares<T>>,

        /// The divisor signs
        pub(crate) divisor_signs: Vec<ModularNumber<T>>,

        /// The dividend signs
        pub(crate) dividend_signs: Vec<ModularNumber<T>>,

        /// The absolute value of divisors
        pub(crate) abs_divisors: Vec<ModularNumber<T>>,

        /// The absolute value of dividends
        pub(crate) abs_dividends: Vec<ModularNumber<T>>,

        /// The signs multiplied
        pub(crate) sign_products: Vec<ModularNumber<T>>,
    }

    /// Waiting for scale operation
    /// Performs comparisons using COMPARE state machine in order to calculate a scale correction.
    pub struct WaitingScaleCompare<T>
    where
        T: SafePrime,
        ShamirSecretSharer<T>: SafePrimeSecretSharer<T>,
    {
        /// The compare state machine
        pub(crate) state_machine: ScaleStateMachine<T>,

        /// The secret sharer
        pub(crate) secret_sharer: Arc<ShamirSecretSharer<T>>,

        /// Security parameter
        pub(crate) security_parameter: usize,

        /// Integer size
        pub(crate) integer_size: usize,

        /// The fixed point precision
        pub(crate) precision: usize,

        /// The dividends
        pub(crate) dividends: Vec<ModularNumber<T>>,

        /// The divisors
        pub(crate) divisors: Vec<ModularNumber<T>>,

        /// Prep compare shares
        pub(crate) prep_elements: Vec<PrepDivisionIntegerSecretShares<T>>,

        /// The signs of divisors
        pub(crate) signs: Vec<ModularNumber<T>>,

        /// The result of the share multiplication protocol
        pub(crate) scales: Vec<ModularNumber<T>>,

        /// 2**(precision+1).
        pub(crate) two_to_exponent: ModularNumber<T>,
    }

    /// Waiting for mult operation with scales
    pub struct WaitingScaleMult<T>
    where
        T: SafePrime,
        ShamirSecretSharer<T>: SafePrimeSecretSharer<T>,
    {
        /// The mult state machine
        pub(crate) state_machine: MultStateMachine<T>,

        /// The secret sharer
        pub(crate) secret_sharer: Arc<ShamirSecretSharer<T>>,

        /// Security parameter
        pub(crate) security_parameter: usize,

        /// Integer size
        pub(crate) integer_size: usize,

        /// The fixed point precision
        pub(crate) precision: usize,

        /// 2**(precision + 1)
        pub(crate) two_to_exponent: ModularNumber<T>,

        /// The dividends
        pub(crate) dividends: Vec<ModularNumber<T>>,

        /// The divisors
        pub(crate) divisors: Vec<ModularNumber<T>>,

        /// Prep compare shares
        pub(crate) prep_elements: Vec<PrepDivisionIntegerSecretShares<T>>,

        /// The signs of divisors
        pub(crate) signs: Vec<ModularNumber<T>>,

        /// The result of the share multiplication protocol
        pub(crate) scaled_divisors: Vec<ModularNumber<T>>,

        /// The result of the share multiplication protocol
        pub(crate) scaled_dividends: Vec<ModularNumber<T>>,
    }

    /// The protocol is waiting for MULTIPLICATION-AND-TRUNCATION operation.
    ///
    /// This state is invoked during the main loop in Newton-Raphson method.
    /// The loop is executed a total number of rounds based on the `precision` parameter.
    /// There are two MULTIPLICATION-AND-TRUNCATION operations per loop. The first one is used to calculate the scaling
    /// parameter `z`. The second one calculates the new estimation of the reciprocal.
    pub struct WaitingMultTrunc<T>
    where
        T: SafePrime,
        ShamirSecretSharer<T>: SafePrimeSecretSharer<T>,
    {
        /// The MULTIPLICATION-AND-TRUNCATION state machine.
        pub(crate) state_machine: MultTruncStateMachine<T>,

        /// The secret sharer
        pub(crate) secret_sharer: Arc<ShamirSecretSharer<T>>,

        /// Security parameter
        pub(crate) security_parameter: usize,

        /// Integer size
        pub(crate) integer_size: usize,

        /// The fixed point precision
        pub(crate) precision: usize,

        /// 2**(precision + 1)
        pub(crate) two_to_exponent: ModularNumber<T>,

        /// The dividends
        pub(crate) dividends: Vec<ModularNumber<T>>,

        /// The divisors
        pub(crate) divisors: Vec<ModularNumber<T>>,

        /// Prep compare shares
        pub(crate) prep_elements: Vec<PrepDivisionIntegerSecretShares<T>>,

        /// The signs of divisors
        pub(crate) signs: Vec<ModularNumber<T>>,

        /// The scaled divisors
        pub(crate) scaled_divisors: Vec<ModularNumber<T>>,

        /// The scaled dividends
        pub(crate) scaled_dividends: Vec<ModularNumber<T>>,

        /// The result of the modulo protocol
        pub(crate) products: Vec<ModularNumber<T>>,

        /// The current iteration reciprocal estimate
        pub(crate) reciprocals: Vec<ModularNumber<T>>,

        /// The loop index
        pub(crate) round_id: u32,

        /// The total loop count
        pub(crate) total_rounds: u32,
    }

    /// Waiting for mult operation of dividend with scales
    pub struct WaitingDivide<T>
    where
        T: SafePrime,
        ShamirSecretSharer<T>: SafePrimeSecretSharer<T>,
    {
        /// The mult state machine
        pub(crate) state_machine: MultTruncStateMachine<T>,

        /// The secret sharer
        pub(crate) secret_sharer: Arc<ShamirSecretSharer<T>>,

        /// Security parameter
        pub(crate) security_parameter: usize,

        /// Integer size
        pub(crate) integer_size: usize,

        /// The fixed point precision
        pub(crate) precision: usize,

        /// The dividends
        pub(crate) dividends: Vec<ModularNumber<T>>,

        /// The divisors
        pub(crate) divisors: Vec<ModularNumber<T>>,

        /// Prep compare shares
        pub(crate) prep_elements: Vec<PrepDivisionIntegerSecretShares<T>>,

        /// The signs of divisors.
        pub(crate) signs: Vec<ModularNumber<T>>,

        /// The result of the division by mult
        pub(crate) quotients: Vec<ModularNumber<T>>,
    }

    /// The protocol is waiting for TRUNC operation.
    pub struct WaitingTrunc<T>
    where
        T: SafePrime,
        ShamirSecretSharer<T>: SafePrimeSecretSharer<T>,
    {
        /// The mult state machine
        pub(crate) state_machine: Modulo2mStateMachine<T>,

        /// The secret sharer
        pub(crate) secret_sharer: Arc<ShamirSecretSharer<T>>,

        /// The dividends
        pub(crate) dividends: Vec<ModularNumber<T>>,

        /// The divisors
        pub(crate) divisors: Vec<ModularNumber<T>>,

        /// Prep compare shares
        pub(crate) prep_elements: Vec<PrepDivisionIntegerSecretShares<T>>,

        /// The signs of divisors
        pub(crate) signs: Vec<ModularNumber<T>>,

        /// The result of the division by mult
        pub(crate) quotients: Vec<ModularNumber<T>>,
    }

    /// The protocol is waiting for MULT operation to calculate estimate and correction.
    pub struct WaitingProduct<T>
    where
        T: SafePrime,
        ShamirSecretSharer<T>: SafePrimeSecretSharer<T>,
    {
        /// The mult state machine
        pub(crate) state_machine: MultStateMachine<T>,

        /// The secret sharer
        pub(crate) secret_sharer: Arc<ShamirSecretSharer<T>>,

        /// The dividends
        pub(crate) dividends: Vec<ModularNumber<T>>,

        /// Prep compare shares
        pub(crate) prep_elements: Vec<PrepDivisionIntegerSecretShares<T>>,

        /// The signs of divisors.
        pub(crate) signs: Vec<ModularNumber<T>>,

        /// The result of the division by mult
        pub(crate) quotients: Vec<ModularNumber<T>>,

        /// The result of the mult operation for estimated dividends
        pub(crate) estimated_dividends: Vec<ModularNumber<T>>,

        /// The result of the mult operation for corrections
        pub(crate) corrections: Vec<ModularNumber<T>>,
    }

    /// The protocol is waiting for COMPARE operation to calculate corrections.
    pub struct WaitingCorrection<T>
    where
        T: SafePrime,
        ShamirSecretSharer<T>: SafePrimeSecretSharer<T>,
    {
        /// The mult state machine
        pub(crate) state_machine: CompareStateMachine<T>,

        /// The secret sharer
        pub(crate) secret_sharer: Arc<ShamirSecretSharer<T>>,

        /// The signs of divisors
        pub(crate) signs: Vec<ModularNumber<T>>,

        /// The result of the division
        pub(crate) quotients: Vec<ModularNumber<T>>,

        /// The result of the compare operation
        pub(crate) low_corrections: Vec<ModularNumber<T>>,

        /// The result of the compare operation
        pub(crate) high_corrections: Vec<ModularNumber<T>>,
    }

    /// The protocol is waiting for MULT operation to correct the sign of the quotient.
    pub struct WaitingFinalSign<T>
    where
        T: SafePrime,
        ShamirSecretSharer<T>: SafePrimeSecretSharer<T>,
    {
        /// The mult state machine
        pub(crate) state_machine: MultStateMachine<T>,

        /// The result of the division
        pub(crate) quotients: Vec<ModularNumber<T>>,
    }
}

/// The input shared dividend and public divisor involved in the integer division operation.
#[derive(Clone, Debug)]
pub struct DivisionIntegerSecretDivisorShares<T>
where
    T: SafePrime,
{
    /// The shared dividend.
    pub dividend: ModularNumber<T>,

    /// The public divisor.
    pub divisor: ModularNumber<T>,

    /// The pre-processing elements
    pub prep_elements: PrepDivisionIntegerSecretShares<T>,
}

/// The state machine for the division integer secret divisor protocol.
#[derive(StateMachineState)]
#[state_machine(
    recipient_id = "PartyId",
    input_message = "PartyMessage<DivisionIntegerSecretDivisorStateMessage>",
    output_message = "DivisionIntegerSecretDivisorStateMessage",
    final_result = "Vec<ModularNumber<T>>",
    handle_message_fn = "Self::handle_message"
)]
pub enum DivisionIntegerSecretDivisorState<T>
where
    T: SafePrime,
    ShamirSecretSharer<T>: SafePrimeSecretSharer<T>,
{
    /// Waiting for sign calculation
    #[state_machine(submachine = "state.state_machine", transition_fn = "Self::transition_waiting_sign_calculation")]
    WaitingSignCalculation(states::WaitingSignCalculation<T>),

    /// Waiting for sign correction operation
    #[state_machine(submachine = "state.state_machine", transition_fn = "Self::transition_waiting_sign_correction")]
    WaitingSignCorrection(states::WaitingSignCorrection<T>),

    /// Waiting for scale compare operation
    #[state_machine(submachine = "state.state_machine", transition_fn = "Self::transition_waiting_scale_compare")]
    WaitingScaleCompare(states::WaitingScaleCompare<T>),

    /// Waiting for scale mult operation
    #[state_machine(submachine = "state.state_machine", transition_fn = "Self::transition_waiting_scale_mult")]
    WaitingScaleMult(states::WaitingScaleMult<T>),

    /// Waiting for MULTIPLICATION-AND-TRUNCATION
    #[state_machine(submachine = "state.state_machine", transition_fn = "Self::transition_waiting_mult_trunc")]
    WaitingMultTrunc(states::WaitingMultTrunc<T>),

    /// Waiting for DIVIDE
    #[state_machine(submachine = "state.state_machine", transition_fn = "Self::transition_waiting_divide")]
    WaitingDivide(states::WaitingDivide<T>),

    /// Waiting for TRUNCPR
    #[state_machine(submachine = "state.state_machine", transition_fn = "Self::transition_waiting_trunc")]
    WaitingTrunc(states::WaitingTrunc<T>),

    /// Waiting for the MULT
    #[state_machine(submachine = "state.state_machine", transition_fn = "Self::transition_waiting_product")]
    WaitingProduct(states::WaitingProduct<T>),

    /// Waiting for COMPARE
    #[state_machine(submachine = "state.state_machine", transition_fn = "Self::transition_waiting_correction")]
    WaitingCorrection(states::WaitingCorrection<T>),

    /// Waiting for MULT
    #[state_machine(submachine = "state.state_machine", transition_fn = "Self::transition_waiting_final_sign")]
    WaitingFinalSign(states::WaitingFinalSign<T>),
}

use crate::division::division_secret_divisor::offline::PrepDivisionIntegerSecretShares;
use DivisionIntegerSecretDivisorState::*;

impl<T> DivisionIntegerSecretDivisorState<T>
where
    T: SafePrime,
    ShamirSecretSharer<T>: SafePrimeSecretSharer<T>,
{
    /// Construct a new DIV-INT-SECRET state.
    pub fn new(
        division_elements: Vec<DivisionIntegerSecretDivisorShares<T>>,
        secret_sharer: Arc<ShamirSecretSharer<T>>,
        kappa: usize,
        k: usize,
    ) -> Result<(Self, Vec<StateMachineMessage<Self>>), DivisionCreateError> {
        let divisors: Vec<_> = division_elements.iter().map(|element| element.divisor).collect();
        let dividends: Vec<_> = division_elements.iter().map(|element| element.dividend).collect();
        let prep_elements: Vec<_> = division_elements.into_iter().map(|element| element.prep_elements).collect();

        // Calculate sign of the input
        let comparands = Self::build_sign_comparands(&divisors, &dividends, &prep_elements)?;
        let (compare_state, messages) = CompareState::new(comparands, secret_sharer.clone())?;
        let next_state = states::WaitingSignCalculation {
            state_machine: StateMachine::new(compare_state),
            secret_sharer,
            security_parameter: kappa,
            integer_size: k,
            divisors,
            dividends,
            prep_elements,
            divisor_signs: Vec::new(),
            dividend_signs: Vec::new(),
        };
        let messages = messages
            .into_iter()
            .map(|message| message.wrap(&DivisionIntegerSecretDivisorStateMessage::SignCalculation))
            .collect();
        Ok((WaitingSignCalculation(next_state), messages))
    }

    #[inline]
    fn build_sign_comparands(
        divisors: &[ModularNumber<T>],
        dividends: &[ModularNumber<T>],
        prep_elements: &[PrepDivisionIntegerSecretShares<T>],
    ) -> Result<Vec<Comparands<T>>, DivisionCreateError> {
        let mut comparands = Vec::new();
        for (divisor, prep_element) in divisors.iter().zip(prep_elements.iter()) {
            let prep_sign = prep_element.prep_compare.first().ok_or(DivisionCreateError::IndexNotFound)?;
            let comparand = Comparands { left: *divisor, right: ModularNumber::ZERO, prep_elements: prep_sign.clone() };
            comparands.push(comparand);
        }
        for (dividend, prep_element) in dividends.iter().zip(prep_elements.iter()) {
            let prep_sign = prep_element.prep_compare.get(1).ok_or(DivisionCreateError::IndexNotFound)?;
            let comparand =
                Comparands { left: *dividend, right: ModularNumber::ZERO, prep_elements: prep_sign.clone() };
            comparands.push(comparand);
        }
        Ok(comparands)
    }

    /// This function is called when the state `WaitingSignCalculation` is transitioned.
    ///
    /// This state is the first one.
    fn transition_waiting_sign_calculation(state: states::WaitingSignCalculation<T>) -> StateMachineStateResult<Self> {
        let mut operands = Vec::new();
        let two = ModularNumber::two();
        for (divisor, sign) in state.divisors.iter().zip(state.divisor_signs.iter()) {
            let sign = ModularNumber::ONE - &(two * sign);
            let operand = OperandShares::single(*divisor, sign);
            operands.push(operand);
        }
        for (dividend, sign) in state.dividends.iter().zip(state.dividend_signs.iter()) {
            let sign = ModularNumber::ONE - &(two * sign);
            let operand = OperandShares::single(*dividend, sign);
            operands.push(operand);
        }
        for (dividend_sign, divisor_sign) in state.dividend_signs.iter().zip(state.divisor_signs.iter()) {
            let operand = OperandShares::single(*dividend_sign, *divisor_sign);
            operands.push(operand);
        }
        let (mult_state, messages) = MultState::new(operands, state.secret_sharer.clone())
            .map_err(|e| anyhow!("failed to create MULT state: {e}"))?;
        let next_state = states::WaitingSignCorrection {
            state_machine: StateMachine::new(mult_state),
            secret_sharer: state.secret_sharer,
            security_parameter: state.security_parameter,
            integer_size: state.integer_size,
            prep_elements: state.prep_elements,
            divisor_signs: state.divisor_signs,
            dividend_signs: state.dividend_signs,
            abs_divisors: Vec::new(),
            abs_dividends: Vec::new(),
            sign_products: Vec::new(),
        };
        let messages = messages
            .into_iter()
            .map(|message| message.wrap(&DivisionIntegerSecretDivisorStateMessage::SignCorrection))
            .collect();
        Ok(StateMachineStateOutput::Messages(WaitingSignCorrection(next_state), messages))
    }

    /// This function is called when the state `WaitingSignCorrection` is transitioned.
    fn transition_waiting_sign_correction(state: states::WaitingSignCorrection<T>) -> StateMachineStateResult<Self> {
        // Part 1: Calculate Signs
        let signs = calculate_signs(state.divisor_signs, state.dividend_signs, state.sign_products);

        // Part 2: Scale
        let operands = build_scale_operands(&state.abs_divisors, &state.prep_elements);
        let precision = state.integer_size / 2;

        let (scale_state, messages) = ScaleState::new(operands, precision, state.secret_sharer.clone())
            .map_err(|e| anyhow!("failed to create SCALE state: {e}"))?;
        let next_state = states::WaitingScaleCompare {
            state_machine: StateMachine::new(scale_state),
            secret_sharer: state.secret_sharer,
            security_parameter: state.security_parameter,
            integer_size: state.integer_size,
            precision,
            divisors: state.abs_divisors,
            dividends: state.abs_dividends,
            prep_elements: state.prep_elements,
            signs,
            scales: Vec::new(),
            two_to_exponent: ModularNumber::ONE,
        };
        let messages = messages
            .into_iter()
            .map(|message| message.wrap(&DivisionIntegerSecretDivisorStateMessage::ScaleCompare))
            .collect();
        Ok(StateMachineStateOutput::Messages(WaitingScaleCompare(next_state), messages))
    }

    /// This function is called when the state `WaitingScaleCompare` is transitioned.
    fn transition_waiting_scale_compare(state: states::WaitingScaleCompare<T>) -> StateMachineStateResult<Self> {
        // Create Mult Operands.
        let operands = build_scale_mult_operands(state.scales, &state.divisors, &state.dividends);

        let (mult_state, messages) = MultState::new(operands, state.secret_sharer.clone())
            .map_err(|e| anyhow!("failed to create MULT state: {e}"))?;
        let next_state = states::WaitingScaleMult {
            state_machine: StateMachine::new(mult_state),
            secret_sharer: state.secret_sharer,
            security_parameter: state.security_parameter,
            integer_size: state.integer_size,
            precision: state.precision,
            two_to_exponent: state.two_to_exponent,
            dividends: state.dividends,
            divisors: state.divisors,
            prep_elements: state.prep_elements,
            signs: state.signs,
            scaled_divisors: Vec::new(),
            scaled_dividends: Vec::new(),
        };
        let messages = messages
            .into_iter()
            .map(|message| message.wrap(&DivisionIntegerSecretDivisorStateMessage::ScaleMult))
            .collect();
        Ok(StateMachineStateOutput::Messages(WaitingScaleMult(next_state), messages))
    }

    fn transition_waiting_scale_mult(state: states::WaitingScaleMult<T>) -> StateMachineStateResult<Self> {
        // The `products` attribute contain the scaled divisors (inputs to Newton-Raphson method).
        // Let's calculate initial guesses.
        let initial_guess = Self::initial_guess(state.precision, &state.scaled_divisors);
        // This is twice `t` as we consider each multiplication-and-truncation in the loop a different round.
        let total_rounds = ((-(state.precision as f64) / ALPHA.log2()).log2().ceil() * 2.0) as u32;
        // Calculate shares for first iteration
        let mult_trunc_shares = Self::calculate_mult_trunc_shares(
            initial_guess.clone(),
            state.scaled_divisors.clone(),
            &state.prep_elements,
            ModularNumber::from_u64(state.precision as u64),
            0,
        )?;
        let (mult_trunc_state, messages) = MultTruncState::new(
            mult_trunc_shares,
            state.secret_sharer.clone(),
            state.security_parameter,
            state.integer_size,
        )
        .map_err(|e| {
            StateMachineError::UnexpectedError(anyhow!(
                "Error creating MULTIPLICATION-AND-TRUNCATION state machine {e}"
            ))
        })?;

        let next_state = states::WaitingMultTrunc {
            state_machine: StateMachine::new(mult_trunc_state),
            secret_sharer: state.secret_sharer,
            security_parameter: state.security_parameter,
            integer_size: state.integer_size,
            precision: state.precision,
            two_to_exponent: state.two_to_exponent,
            dividends: state.dividends,
            divisors: state.divisors,
            prep_elements: state.prep_elements,
            signs: state.signs,
            reciprocals: initial_guess,
            scaled_divisors: state.scaled_divisors,
            scaled_dividends: state.scaled_dividends,
            products: Vec::new(),
            round_id: 0,
            total_rounds,
        };
        let messages = messages
            .into_iter()
            .map(|message| message.wrap(&|message| DivisionIntegerSecretDivisorStateMessage::MultTrunc(message, 0)))
            .collect();
        Ok(StateMachineStateOutput::Messages(WaitingMultTrunc(next_state), messages))
    }

    #[inline]
    /// Initial guess for Newton-Raphson method
    fn initial_guess(precision: usize, divisors: &[ModularNumber<T>]) -> Vec<ModularNumber<T>> {
        // 2**precision == 1 << (precision)
        let precision_power = 2f64.powf(precision as f64);
        let c_initial = (3f64 - ALPHA) * precision_power;
        let c_initial = ModularNumber::from_u64(c_initial.round() as u64);
        divisors.iter().map(|divisor| c_initial - &(ModularNumber::two() * divisor)).collect()
    }

    #[inline]
    /// Calculates MULTIPLICATION-AND-TRUNCATION shares for `index`.
    ///
    /// Each division requires a set of pre-processing shares, which are mainly 1 PREP-COMPUTE, 2*f+2 PREP-TRUNCPR.
    /// In Newton-Raphson, we iterate `f` times, performing 2 MULTIPLICATION-AND-TRUNCATION each iteration. Then we do one more MULTIPLICATION-AND-TRUNCATION and also a TRUNCPR.
    ///
    /// We need to collect the corresponding indexes for each operation. For the first operation index is 0,
    /// and corresponds to the first operation in the first loop iteration. The second operation has index 1. For the second loop iteration,
    /// the first operation will have index 2 and so on.
    /// The last operation has index 2*f+1.
    fn calculate_mult_trunc_shares(
        lefts: Vec<ModularNumber<T>>,
        rights: Vec<ModularNumber<T>>,
        prep_elements: &[PrepDivisionIntegerSecretShares<T>],
        trunc_exponent: ModularNumber<T>,
        index: usize,
    ) -> Result<Vec<MultTruncShares<T>>, StateMachineError> {
        let mut shares = Vec::new();
        for ((left, right), prep_element) in lefts.into_iter().zip(rights).zip(prep_elements.iter()) {
            // We select the shares for the index given.
            let prep_elements = prep_element.prep_truncpr.get(index).ok_or_else(|| anyhow!("index not found"))?.clone();
            let share = MultTruncShares { left, right, prep_elements, trunc_exponent };
            shares.push(share);
        }
        Ok(shares)
    }

    /// This function is called when the state `WaitingMultTrunc` is transitioned.
    ///
    /// This happens twice for every loop iteration in the Newton-Raphson method.
    /// - In the first pass, we calculate truncPR(c * b, f) to get `z` = 2**(f+1) - truncPR(c * b, f)
    /// - In the second pass, we calculate the current reciprocal estimate `c` = truncPR(c * z, f)
    fn transition_waiting_mult_trunc(state: states::WaitingMultTrunc<T>) -> StateMachineStateResult<Self> {
        let round_id = state.round_id.checked_add(1).ok_or_else(|| anyhow!("integer overflow"))?;
        let first = (round_id % 2) == 0;
        // We've reached the end of the loop. Move to next state.
        // The product attribute contains the final estimate for the reciprocal.
        if round_id == state.total_rounds {
            return Self::transition_to_divide(state);
        }
        let (reciprocals, rights) = if first {
            // For every first pass we set a new value of reciprocal based on the product c*z
            (state.products, state.scaled_divisors.clone())
        } else {
            // Right is the parameter z = 2**(f+1) - MULTIPLICATION-AND-TRUNCATION(c*b)
            (state.reciprocals, state.products.into_iter().map(|product| state.two_to_exponent - &product).collect())
        };
        // For the first pass, c*b, for the second pass, c*z
        let mult_trunc_shares = Self::calculate_mult_trunc_shares(
            reciprocals.clone(),
            rights,
            &state.prep_elements,
            ModularNumber::from_u64(state.precision as u64),
            round_id as usize,
        )?;
        let (mult_trunc_state, messages) = MultTruncState::new(
            mult_trunc_shares,
            state.secret_sharer.clone(),
            state.security_parameter,
            state.integer_size,
        )
        .map_err(|e| {
            StateMachineError::UnexpectedError(anyhow!(
                "Error creating MULTIPLICATION-AND-TRUNCATION state machine {e}"
            ))
        })?;
        let next_state = states::WaitingMultTrunc {
            state_machine: StateMachine::new(mult_trunc_state),
            secret_sharer: state.secret_sharer,
            security_parameter: state.security_parameter,
            integer_size: state.integer_size,
            precision: state.precision,
            two_to_exponent: state.two_to_exponent,
            dividends: state.dividends,
            divisors: state.divisors,
            prep_elements: state.prep_elements,
            signs: state.signs,
            reciprocals,
            scaled_divisors: state.scaled_divisors,
            scaled_dividends: state.scaled_dividends,
            products: Vec::new(),
            round_id,
            total_rounds: state.total_rounds,
        };
        let messages = messages
            .into_iter()
            .map(|message| {
                message.wrap(&|message| DivisionIntegerSecretDivisorStateMessage::MultTrunc(message, round_id))
            })
            .collect();
        Ok(StateMachineStateOutput::Messages(WaitingMultTrunc(next_state), messages))
    }

    fn transition_to_divide(state: states::WaitingMultTrunc<T>) -> StateMachineStateResult<Self> {
        // Create Mult Trunc Operands.
        let operands = Self::calculate_mult_trunc_shares(
            state.products,
            state.scaled_dividends,
            &state.prep_elements,
            ModularNumber::from_u64(state.precision as u64),
            state.total_rounds as usize,
        )?;
        let (mult_state, messages) =
            MultTruncState::new(operands, state.secret_sharer.clone(), state.security_parameter, state.integer_size)
                .map_err(|e| anyhow!("failed to create MULTIPLICATION-AND-TRUNCATION state: {e}"))?;
        let next_state = states::WaitingDivide {
            state_machine: StateMachine::new(mult_state),
            secret_sharer: state.secret_sharer,
            security_parameter: state.security_parameter,
            integer_size: state.integer_size,
            precision: state.precision,
            dividends: state.dividends,
            divisors: state.divisors,
            prep_elements: state.prep_elements,
            signs: state.signs,
            quotients: Vec::new(),
        };
        let messages = messages
            .into_iter()
            .map(|message| message.wrap(&DivisionIntegerSecretDivisorStateMessage::Divide))
            .collect();
        Ok(StateMachineStateOutput::Messages(WaitingDivide(next_state), messages))
    }

    fn transition_waiting_divide(state: states::WaitingDivide<T>) -> StateMachineStateResult<Self> {
        let modular_precision = ModularNumber::from_u64(state.precision as u64);
        let trunc_elements = state
            .quotients
            .into_iter()
            .zip(state.prep_elements.clone())
            .map(|(quotient, prep_element)| Modulo2mShares {
                dividend: quotient,
                divisors_exp_m: modular_precision,
                prep_elements: prep_element.prep_trunc.clone(),
            })
            .collect();
        let (trunc_state, messages) = Modulo2mState::new(
            trunc_elements,
            state.secret_sharer.clone(),
            state.security_parameter,
            state.integer_size,
            Mod2mTruncVariant::Trunc,
        )
        .map_err(|e| StateMachineError::UnexpectedError(anyhow!("Error creating TRUNC state machine {e}")))?;
        let next_state = states::WaitingTrunc {
            state_machine: StateMachine::new(trunc_state),
            secret_sharer: state.secret_sharer,
            divisors: state.divisors,
            dividends: state.dividends,
            prep_elements: state.prep_elements,
            signs: state.signs,
            quotients: Vec::new(),
        };
        let messages = messages
            .into_iter()
            .map(|message| message.wrap(&DivisionIntegerSecretDivisorStateMessage::Trunc))
            .collect();
        Ok(StateMachineStateOutput::Messages(WaitingTrunc(next_state), messages))
    }

    /// This function is called when the state `WaitingTrunc` is transitioned.
    fn transition_waiting_trunc(state: states::WaitingTrunc<T>) -> StateMachineStateResult<Self> {
        let mut operands = Vec::new();
        for (quotient, divisor) in state.quotients.iter().zip(state.divisors.iter()) {
            let operand = OperandShares::single(*quotient, *divisor);
            operands.push(operand);
        }
        for (sign, divisor) in state.signs.iter().zip(state.divisors.iter()) {
            let right = divisor - &ModularNumber::ONE;
            let operand = OperandShares::single(*sign, right);
            operands.push(operand);
        }
        let (mult_state, messages) = MultState::new(operands, state.secret_sharer.clone())
            .map_err(|e| anyhow!("failed to create MULT state: {e}"))?;
        let next_state = states::WaitingProduct {
            state_machine: StateMachine::new(mult_state),
            secret_sharer: state.secret_sharer,
            dividends: state.dividends,
            prep_elements: state.prep_elements,
            signs: state.signs,
            quotients: state.quotients,
            estimated_dividends: Vec::new(),
            corrections: Vec::new(),
        };
        let messages = messages
            .into_iter()
            .map(|message| message.wrap(&DivisionIntegerSecretDivisorStateMessage::Product))
            .collect();
        Ok(StateMachineStateOutput::Messages(WaitingProduct(next_state), messages))
    }

    /// This function is called when the state `WaitingProduct` is transitioned.
    fn transition_waiting_product(state: states::WaitingProduct<T>) -> StateMachineStateResult<Self> {
        let mut comparands = Vec::new();
        for ((estimate, dividend), prep) in
            state.estimated_dividends.iter().zip(state.dividends.iter()).zip(state.prep_elements.iter())
        {
            let prep_elements =
                prep.prep_compare.get(2).ok_or_else(|| anyhow!("compare element not found in DIVISION"))?;
            let comparand = Comparands { left: *dividend, right: *estimate, prep_elements: prep_elements.clone() };
            comparands.push(comparand)
        }
        for (((estimate, correction), dividend), prep) in state
            .estimated_dividends
            .iter()
            .zip(state.corrections.iter())
            .zip(state.dividends.iter())
            .zip(state.prep_elements.iter())
        {
            let prep_elements =
                prep.prep_compare.get(3).ok_or_else(|| anyhow!("compare element not found in DIVISION"))?;
            let left = estimate + correction;
            let comparand = Comparands { left, right: *dividend, prep_elements: prep_elements.clone() };
            comparands.push(comparand)
        }
        let (compare_state, messages) = CompareState::new(comparands, state.secret_sharer.clone())
            .map_err(|e| anyhow!("failed to create COMPARE state: {e}"))?;
        let next_state = states::WaitingCorrection {
            state_machine: StateMachine::new(compare_state),
            secret_sharer: state.secret_sharer,
            signs: state.signs,
            quotients: state.quotients,
            low_corrections: Vec::new(),
            high_corrections: Vec::new(),
        };
        let messages = messages
            .into_iter()
            .map(|message| message.wrap(&DivisionIntegerSecretDivisorStateMessage::Correction))
            .collect();
        Ok(StateMachineStateOutput::Messages(WaitingCorrection(next_state), messages))
    }

    /// This function is called when the state `WaitingCorrection` is transitioned.
    fn transition_waiting_correction(state: states::WaitingCorrection<T>) -> StateMachineStateResult<Self> {
        let mut operands = Vec::new();
        let two = ModularNumber::two();
        for (((quotient, sign), low), high) in state
            .quotients
            .iter()
            .zip(state.signs.iter())
            .zip(state.low_corrections.iter())
            .zip(state.high_corrections.iter())
        {
            let quotient = quotient - low + high;
            let sign = (two * sign) - &ModularNumber::ONE;
            let operand = OperandShares::single(quotient, sign);
            operands.push(operand);
        }
        let (mult_state, messages) = MultState::new(operands, state.secret_sharer.clone())
            .map_err(|e| anyhow!("failed to create MULT state: {e}"))?;
        let next_state =
            states::WaitingFinalSign { state_machine: StateMachine::new(mult_state), quotients: Vec::new() };
        let messages = messages
            .into_iter()
            .map(|message| message.wrap(&DivisionIntegerSecretDivisorStateMessage::FinalSign))
            .collect();
        Ok(StateMachineStateOutput::Messages(WaitingFinalSign(next_state), messages))
    }

    fn transition_waiting_final_sign(state: states::WaitingFinalSign<T>) -> StateMachineStateResult<Self> {
        Ok(StateMachineStateOutput::Final(state.quotients))
    }

    fn handle_message(
        mut state: Self,
        message: PartyMessage<DivisionIntegerSecretDivisorStateMessage>,
    ) -> StateMachineStateResult<Self> {
        use DivisionIntegerSecretDivisorStateMessage::*;
        let (party_id, message) = message.into_parts();
        match (message, &mut state) {
            (SignCalculation(message), WaitingSignCalculation(inner)) => {
                match inner.state_machine.handle_message(PartyMessage::new(party_id, message))? {
                    StateMachineOutput::Final(values) => {
                        let mid = values.len() / 2;
                        inner.divisor_signs =
                            values.get(..mid).ok_or_else(|| anyhow!("Invalid index in DIVIDE"))?.to_vec();
                        inner.dividend_signs =
                            values.get(mid..).ok_or_else(|| anyhow!("Invalid index in DIVIDE"))?.to_vec();
                        if inner.divisor_signs.len() != inner.dividend_signs.len() {
                            return Err(anyhow!("The two sign vectors in DIVIDE have different sizes").into());
                        }
                        state.try_next()
                    }
                    output => state.wrap_message(output, DivisionIntegerSecretDivisorStateMessage::SignCalculation),
                }
            }
            (SignCorrection(message), WaitingSignCorrection(inner)) => {
                match inner.state_machine.handle_message(PartyMessage::new(party_id, message))? {
                    StateMachineOutput::Final(values) => {
                        let third = values.len() / 3;
                        // Safety: this is less than values, overflow can't happen.
                        let two_thirds = third * 2;
                        inner.abs_divisors =
                            values.get(..third).ok_or_else(|| anyhow!("Invalid index in DIVIDE"))?.to_vec();
                        inner.abs_dividends =
                            values.get(third..two_thirds).ok_or_else(|| anyhow!("Invalid index in DIVIDE"))?.to_vec();
                        inner.sign_products =
                            values.get(two_thirds..).ok_or_else(|| anyhow!("Invalid index in DIVIDE"))?.to_vec();
                        if inner.abs_divisors.len() != inner.abs_dividends.len() {
                            return Err(anyhow!("The two sign vectors in DIVIDE have different sizes").into());
                        }
                        if inner.abs_divisors.len() != inner.sign_products.len() {
                            return Err(anyhow!("The two sign vectors in DIVIDE have different sizes").into());
                        }
                        state.try_next()
                    }
                    output => state.wrap_message(output, DivisionIntegerSecretDivisorStateMessage::SignCorrection),
                }
            }
            (ScaleCompare(message), WaitingScaleCompare(inner)) => {
                match inner.state_machine.handle_message(PartyMessage::new(party_id, message))? {
                    StateMachineOutput::Final((scales, two_to_exponent)) => {
                        inner.scales = scales;
                        inner.two_to_exponent = two_to_exponent;
                        state.try_next()
                    }
                    output => state.wrap_message(output, DivisionIntegerSecretDivisorStateMessage::ScaleCompare),
                }
            }
            (ScaleMult(message), WaitingScaleMult(inner)) => {
                match inner.state_machine.handle_message(PartyMessage::new(party_id, message))? {
                    StateMachineOutput::Final(values) => {
                        let mid = values.len() / 2;
                        inner.scaled_divisors =
                            values.get(..mid).ok_or_else(|| anyhow!("Invalid index in DIVIDE"))?.to_vec();
                        inner.scaled_dividends =
                            values.get(mid..).ok_or_else(|| anyhow!("Invalid index in DIVIDE"))?.to_vec();
                        if inner.scaled_divisors.len() != inner.scaled_dividends.len() {
                            return Err(anyhow!("The two multiplication vectors in DIVIDE have different sizes").into());
                        }
                        state.try_next()
                    }
                    output => state.wrap_message(output, DivisionIntegerSecretDivisorStateMessage::ScaleMult),
                }
            }
            (MultTrunc(message, round_id), WaitingMultTrunc(inner)) if round_id == inner.round_id => {
                match inner.state_machine.handle_message(PartyMessage::new(party_id, message))? {
                    StateMachineOutput::Final(values) => {
                        inner.products = values;
                        state.try_next()
                    }
                    output => state.wrap_message(output, |message| {
                        DivisionIntegerSecretDivisorStateMessage::MultTrunc(message, round_id)
                    }),
                }
            }
            (Divide(message), WaitingDivide(inner)) => {
                match inner.state_machine.handle_message(PartyMessage::new(party_id, message))? {
                    StateMachineOutput::Final(values) => {
                        inner.quotients = values;
                        state.try_next()
                    }
                    output => state.wrap_message(output, DivisionIntegerSecretDivisorStateMessage::Divide),
                }
            }
            (Trunc(message), WaitingTrunc(inner)) => {
                match inner.state_machine.handle_message(PartyMessage::new(party_id, message))? {
                    StateMachineOutput::Final(values) => {
                        inner.quotients = values;
                        state.try_next()
                    }
                    output => state.wrap_message(output, DivisionIntegerSecretDivisorStateMessage::Trunc),
                }
            }
            (Product(message), WaitingProduct(inner)) => {
                match inner.state_machine.handle_message(PartyMessage::new(party_id, message))? {
                    StateMachineOutput::Final(values) => {
                        let mid = values.len() / 2;
                        inner.estimated_dividends =
                            values.get(..mid).ok_or_else(|| anyhow!("Invalid index in DIVIDE"))?.to_vec();
                        inner.corrections =
                            values.get(mid..).ok_or_else(|| anyhow!("Invalid index in DIVIDE"))?.to_vec();
                        if inner.estimated_dividends.len() != inner.corrections.len() {
                            return Err(anyhow!("The two multiplication vectors in DIVIDE have different sizes").into());
                        }
                        state.try_next()
                    }
                    output => state.wrap_message(output, DivisionIntegerSecretDivisorStateMessage::Product),
                }
            }
            (Correction(message), WaitingCorrection(inner)) => {
                match inner.state_machine.handle_message(PartyMessage::new(party_id, message))? {
                    StateMachineOutput::Final(values) => {
                        let mid = values.len() / 2;
                        inner.low_corrections =
                            values.get(..mid).ok_or_else(|| anyhow!("Invalid index in DIVIDE"))?.to_vec();
                        inner.high_corrections =
                            values.get(mid..).ok_or_else(|| anyhow!("Invalid index in DIVIDE"))?.to_vec();
                        if inner.low_corrections.len() != inner.high_corrections.len() {
                            return Err(anyhow!("The two comparison vectors in DIVIDE have different sizes").into());
                        }
                        state.try_next()
                    }
                    output => state.wrap_message(output, DivisionIntegerSecretDivisorStateMessage::Correction),
                }
            }
            (FinalSign(message), WaitingFinalSign(inner)) => {
                match inner.state_machine.handle_message(PartyMessage::new(party_id, message))? {
                    StateMachineOutput::Final(values) => {
                        inner.quotients = values;
                        state.try_next()
                    }
                    output => state.wrap_message(output, DivisionIntegerSecretDivisorStateMessage::FinalSign),
                }
            }
            (message, _) => Ok(StateMachineStateOutput::OutOfOrder(state, PartyMessage::new(party_id, message))),
        }
    }
}

/// A message for this state machine.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[repr(u8)]
pub enum DivisionIntegerSecretDivisorStateMessage {
    /// A message for the COMPARE state machine
    SignCalculation(CompareStateMessage) = 0,

    /// A message for the MULT state machine
    SignCorrection(MultStateMessage) = 1,

    /// A message for the COMPARE state machine
    ScaleCompare(ScaleStateMessage) = 2,

    /// A message for the MULT state machine
    ScaleMult(MultStateMessage) = 3,

    /// A message for the MULTIPLICATION-AND-TRUNCATION state machine.
    MultTrunc(MultTruncStateMessage, u32) = 4,

    /// A message for the MULT state machine
    Divide(MultTruncStateMessage) = 5,

    /// A message for the TRUNC state machine
    Trunc(Modulo2mStateMessage) = 6,

    /// A message for the MULT state machine
    Product(MultStateMessage) = 7,

    /// A message for the COMPARE state machine
    Correction(CompareStateMessage) = 8,

    /// A message for the MULT state machine
    FinalSign(MultStateMessage) = 9,
}

/// An error during the DIVISION state construction.
#[derive(thiserror::Error, Debug)]
pub enum DivisionCreateError {
    /// An error during the TRUNC operation.
    #[error("MULTIPLICATION-AND-TRUNCATION protocol create error: {0}")]
    MultTrunc(#[from] MultTruncCreateError),

    /// An arithmetic error.
    #[error("arithmetic: {0}")]
    Arithmetic(#[from] DivByZero),

    /// Error while calculating precision
    #[error("calculating precision for {0}")]
    Precision(String),

    /// Error while creating COMPARE state machine
    #[error("failed to create COMPARE state: {0}")]
    Compare(#[from] CompareCreateError),

    /// Index not found
    #[error("index not found")]
    IndexNotFound,
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use math_lib::modular::U128SafePrime;
    use num_bigint::BigInt;

    use super::*;

    #[test]
    fn test_initial_guess() {
        let minus_two = ModularNumber::try_from(&BigInt::from_str("-2").unwrap()).unwrap();
        let numbers: Vec<ModularNumber<U128SafePrime>> = vec![ModularNumber::from_str("1001").unwrap(), minus_two];
        let sign_correction = DivisionIntegerSecretDivisorState::initial_guess(10, &numbers);
        assert_eq!(ModularNumber::from_u32(982), sign_correction[0]);
        assert_eq!(ModularNumber::from_u32(2988), sign_correction[1]);
    }
}
