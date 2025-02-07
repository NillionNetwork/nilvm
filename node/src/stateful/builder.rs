use super::sm::{StandardStateMachine, StateMachine};
use math_lib::modular::{EncodedModularNumber, SafePrime};
use mpc_vm::{
    protocols::MPCProtocol,
    vm::{
        get_statistic_k, plan::MPCRuntimePreprocessingElements, ExecutionMetricsConfig, ExecutionVmConfig,
        MPCExecutionVmMessage, STATISTIC_KAPPA,
    },
    Program,
};
use nada_value::{
    encoders::Decoder,
    encrypted::{Encoded, Encrypted},
    errors::DecodingError,
    validation::{validate_encrypted_values, EncryptedValueValidationError},
    NadaValue,
};
use node_api::preprocessing::proto::stream::AuxiliaryMaterialStreamMessage;
use protocols::{
    conditionals::{
        equality::{
            offline::{
                EncodedPrepPrivateOutputEqualityShares, PrepPrivateOutputEqualityState,
                PrepPrivateOutputEqualityStateMessage,
            },
            POLY_EVAL_DEGREE,
        },
        equality_public_output::offline::{
            state::{PrepPublicOutputEqualityState, PrepPublicOutputEqualityStateMessage},
            EncodedPrepPublicOutputEqualityShares,
        },
        less_than::offline::{
            state::{PrepCompareState, PrepCompareStateMessage},
            EncodedPrepCompareShares,
        },
    },
    division::{
        division_secret_divisor::offline::{
            state::{PrepDivisionIntegerSecretState, PrepDivisionIntegerSecretStateMessage},
            EncodedPrepDivisionIntegerSecretShares,
        },
        modulo2m_public_divisor::offline::{
            state::{PrepModulo2mState, PrepModulo2mStateMessage},
            EncodedPrepModulo2mShares,
        },
        modulo_public_divisor::offline::{
            state::{PrepModuloState, PrepModuloStateMessage},
            EncodedPrepModuloShares,
        },
        truncation_probabilistic::offline::{
            state::{PrepTruncPrState, PrepTruncPrStateMessage},
            EncodedPrepTruncPrShares,
        },
    },
    random::{
        random_bit::{EncodedBitShare, RandomBitState, RandomBitStateMessage},
        random_integer::{RandomIntegerState, RandomIntegerStateMessage, RandomMode},
    },
    threshold_ecdsa::auxiliary_information::{
        output::EcdsaAuxInfo, EcdsaAuxInfoState, EcdsaAuxInfoStateMessage, PregeneratedPrimesMode,
    },
};
use shamir_sharing::secret_sharer::{SafePrimeSecretSharer, SecretSharerProperties, ShamirSecretSharer};
use std::{collections::HashMap, sync::Arc};
use uuid::Uuid;

pub(crate) type ExecutionVm =
    Box<dyn StateMachine<Result = HashMap<String, NadaValue<Encrypted<Encoded>>>, Message = MPCExecutionVmMessage>>;

pub(crate) type PrepCompareStateMachine =
    Box<dyn StateMachine<Result = anyhow::Result<Vec<EncodedPrepCompareShares>>, Message = PrepCompareStateMessage>>;

pub(crate) type PrepDivisionIntegerSecretStateMachine = Box<
    dyn StateMachine<
            Result = anyhow::Result<Vec<EncodedPrepDivisionIntegerSecretShares>>,
            Message = PrepDivisionIntegerSecretStateMessage,
        >,
>;

pub(crate) type PrepModuloStateMachine =
    Box<dyn StateMachine<Result = anyhow::Result<Vec<EncodedPrepModuloShares>>, Message = PrepModuloStateMessage>>;

pub(crate) type PrepPublicOutputEqualityStateMachine = Box<
    dyn StateMachine<
            Result = anyhow::Result<Vec<EncodedPrepPublicOutputEqualityShares>>,
            Message = PrepPublicOutputEqualityStateMessage,
        >,
>;

pub(crate) type PrepEqualsIntegerSecretStateMachine = Box<
    dyn StateMachine<
            Result = anyhow::Result<Vec<EncodedPrepPrivateOutputEqualityShares>>,
            Message = PrepPrivateOutputEqualityStateMessage,
        >,
>;

pub(crate) type PrepTruncPrStateMachine =
    Box<dyn StateMachine<Result = anyhow::Result<Vec<EncodedPrepTruncPrShares>>, Message = PrepTruncPrStateMessage>>;

pub(crate) type PrepTruncStateMachine =
    Box<dyn StateMachine<Result = anyhow::Result<Vec<EncodedPrepModulo2mShares>>, Message = PrepModulo2mStateMessage>>;

pub(crate) type RandomIntegerStateMachine =
    Box<dyn StateMachine<Result = anyhow::Result<Vec<EncodedModularNumber>>, Message = RandomIntegerStateMessage>>;

pub(crate) type RandomBooleanStateMachine =
    Box<dyn StateMachine<Result = anyhow::Result<Vec<EncodedBitShare>>, Message = RandomBitStateMessage>>;

pub(crate) type Cggmp21AuxInfoStateMachine =
    Box<dyn StateMachine<Result = anyhow::Result<Vec<EcdsaAuxInfo>>, Message = EcdsaAuxInfoStateMessage>>;

#[cfg_attr(test, mockall::automock)]
pub(crate) trait PrimeBuilder: Send + Sync + 'static {
    fn build_execution_vm(
        &self,
        program: Program<MPCProtocol>,
        values: HashMap<String, NadaValue<Encrypted<Encoded>>>,
        preprocessing_elements: MPCRuntimePreprocessingElements,
        compute_id: Uuid,
    ) -> Result<ExecutionVm, BuildExecutionVmError>;

    fn build_prep_compare_state_machine(&self, batch_size: usize) -> anyhow::Result<PrepCompareStateMachine>;

    fn build_prep_division_secret_divisor_state_machine(
        &self,
        batch_size: usize,
    ) -> anyhow::Result<PrepDivisionIntegerSecretStateMachine>;

    fn build_prep_modulo_state_machine(&self, batch_size: usize) -> anyhow::Result<PrepModuloStateMachine>;

    fn build_prep_equality_public_output_state_machine(
        &self,
        batch_size: usize,
    ) -> anyhow::Result<PrepPublicOutputEqualityStateMachine>;

    fn build_prep_equality_secret_output_state_machine(
        &self,
        batch_size: usize,
    ) -> anyhow::Result<PrepEqualsIntegerSecretStateMachine>;

    fn build_prep_trunc_pr_state_machine(&self, batch_size: usize) -> anyhow::Result<PrepTruncPrStateMachine>;

    fn build_prep_trunc_state_machine(&self, batch_size: usize) -> anyhow::Result<PrepTruncStateMachine>;

    fn build_random_integer_state_machine(&self, batch_size: usize) -> anyhow::Result<RandomIntegerStateMachine>;

    fn build_random_boolean_state_machine(&self, batch_size: usize) -> anyhow::Result<RandomBooleanStateMachine>;

    fn build_cggmp21_aux_info_state_machine(&self, execution_id: Vec<u8>)
    -> anyhow::Result<Cggmp21AuxInfoStateMachine>;
}

pub(crate) struct DefaultPrimeBuilder<T: SafePrime> {
    sharer: Arc<ShamirSecretSharer<T>>,
    execution_vm_config: ExecutionVmConfig,
}

impl<T: SafePrime> DefaultPrimeBuilder<T> {
    pub(crate) fn new(sharer: ShamirSecretSharer<T>, execution_vm_config: ExecutionVmConfig) -> Self {
        Self { sharer: Arc::new(sharer), execution_vm_config }
    }
}

impl<T> PrimeBuilder for DefaultPrimeBuilder<T>
where
    T: SafePrime,
    ShamirSecretSharer<T>: SafePrimeSecretSharer<T>,
{
    fn build_execution_vm(
        &self,
        program: Program<MPCProtocol>,
        values: HashMap<String, NadaValue<Encrypted<Encoded>>>,
        preprocessing_elements: MPCRuntimePreprocessingElements,
        compute_id: Uuid,
    ) -> Result<ExecutionVm, BuildExecutionVmError> {
        let values = values.decode::<T>()?;
        let inputs = program.contract.inputs_iter().map(|input| (input.name.clone(), input.ty.clone())).collect();
        validate_encrypted_values(&values, &inputs)?;

        let vm = mpc_vm::vm::ExecutionVm::new(
            compute_id,
            &self.execution_vm_config,
            program,
            self.sharer.local_party_id().clone(),
            self.sharer.clone(),
            values,
            preprocessing_elements,
            ExecutionMetricsConfig::disabled(),
        )
        .map_err(|e| BuildExecutionVmError::CreatingVm(e.to_string()))?;
        Ok(Box::new(vm))
    }

    fn build_prep_compare_state_machine(&self, batch_size: usize) -> anyhow::Result<PrepCompareStateMachine> {
        let (state, initial_messages) = PrepCompareState::new(batch_size, self.sharer.clone())?;
        let sm = state_machine::StateMachine::new(state);
        Ok(Box::new(StandardStateMachine::new(sm, initial_messages)))
    }

    fn build_prep_division_secret_divisor_state_machine(
        &self,
        batch_size: usize,
    ) -> anyhow::Result<PrepDivisionIntegerSecretStateMachine> {
        let (state, initial_messages) = PrepDivisionIntegerSecretState::new(
            batch_size,
            STATISTIC_KAPPA,
            get_statistic_k::<T>(),
            self.sharer.clone(),
        )?;
        let sm = state_machine::StateMachine::new(state);
        Ok(Box::new(StandardStateMachine::new(sm, initial_messages)))
    }

    fn build_prep_modulo_state_machine(&self, batch_size: usize) -> anyhow::Result<PrepModuloStateMachine> {
        let (state, initial_messages) =
            PrepModuloState::new(batch_size, STATISTIC_KAPPA, get_statistic_k::<T>(), self.sharer.clone())?;
        let sm = state_machine::StateMachine::new(state);
        Ok(Box::new(StandardStateMachine::new(sm, initial_messages)))
    }

    fn build_prep_equality_public_output_state_machine(
        &self,
        batch_size: usize,
    ) -> anyhow::Result<PrepPublicOutputEqualityStateMachine> {
        let (state, initial_messages) = PrepPublicOutputEqualityState::new(batch_size, self.sharer.clone())?;
        let sm = state_machine::StateMachine::new(state);
        Ok(Box::new(StandardStateMachine::new(sm, initial_messages)))
    }

    fn build_prep_equality_secret_output_state_machine(
        &self,
        batch_size: usize,
    ) -> anyhow::Result<PrepEqualsIntegerSecretStateMachine> {
        let (state, initial_messages) =
            PrepPrivateOutputEqualityState::new(batch_size, POLY_EVAL_DEGREE, self.sharer.clone())?;
        let sm = state_machine::StateMachine::new(state);
        Ok(Box::new(StandardStateMachine::new(sm, initial_messages)))
    }

    fn build_prep_trunc_pr_state_machine(&self, batch_size: usize) -> anyhow::Result<PrepTruncPrStateMachine> {
        let (state, initial_messages) =
            PrepTruncPrState::new(batch_size, STATISTIC_KAPPA, get_statistic_k::<T>(), self.sharer.clone())?;
        let sm = state_machine::StateMachine::new(state);
        Ok(Box::new(StandardStateMachine::new(sm, initial_messages)))
    }

    fn build_prep_trunc_state_machine(&self, batch_size: usize) -> anyhow::Result<PrepTruncStateMachine> {
        let (state, initial_messages) =
            PrepModulo2mState::new(batch_size, STATISTIC_KAPPA, get_statistic_k::<T>(), self.sharer.clone())?;
        let sm = state_machine::StateMachine::new(state);
        Ok(Box::new(StandardStateMachine::new(sm, initial_messages)))
    }

    fn build_random_integer_state_machine(&self, batch_size: usize) -> anyhow::Result<RandomIntegerStateMachine> {
        let (state, initial_messages) =
            RandomIntegerState::new(RandomMode::RandomOfDegreeT, batch_size, self.sharer.clone())?;
        let sm = state_machine::StateMachine::new(state);
        Ok(Box::new(StandardStateMachine::new(sm, initial_messages)))
    }

    fn build_random_boolean_state_machine(&self, batch_size: usize) -> anyhow::Result<RandomBooleanStateMachine> {
        let (state, initial_messages) = RandomBitState::new(batch_size, self.sharer.clone())?;
        let sm = state_machine::StateMachine::new(state);
        Ok(Box::new(StandardStateMachine::new(sm, initial_messages)))
    }

    fn build_cggmp21_aux_info_state_machine(
        &self,
        execution_id: Vec<u8>,
    ) -> anyhow::Result<Cggmp21AuxInfoStateMachine> {
        let (state, initial_messages) = EcdsaAuxInfoState::new(
            execution_id,
            self.sharer.parties(),
            self.sharer.local_party_id().clone(),
            PregeneratedPrimesMode::Random,
        )?;
        let sm = state_machine::StateMachine::new(state);
        Ok(Box::new(StandardStateMachine::<EcdsaAuxInfoState, AuxiliaryMaterialStreamMessage>::new(
            sm,
            initial_messages,
        )))
    }
}

#[derive(Debug, thiserror::Error)]
pub(crate) enum BuildExecutionVmError {
    #[error("invalid values: {0}")]
    InvalidValues(#[from] DecodingError),

    #[error("input validation failed: {0}")]
    InputValidation(#[from] EncryptedValueValidationError),

    #[error("failed to create VM: {0}")]
    CreatingVm(String),
}

#[cfg(test)]
mod tests {
    use super::*;
    use basic_types::PartyId;
    use math_lib::modular::{EncodedModularNumber, EncodedModulo, U64SafePrime};
    use test_programs::PROGRAMS;

    fn make_builder() -> DefaultPrimeBuilder<U64SafePrime> {
        let parties = vec![PartyId::from(vec![1]), PartyId::from(vec![2]), PartyId::from(vec![3])];
        let sharer = ShamirSecretSharer::<U64SafePrime>::new(PartyId::from(vec![]), 1, parties).unwrap();
        DefaultPrimeBuilder::new(sharer, Default::default())
    }

    #[test]
    fn invalid_inputs() {
        let builder = make_builder();
        let values: HashMap<String, NadaValue<Encrypted<Encoded>>> = HashMap::from([(
            "I01".to_string(),
            NadaValue::new_integer(EncodedModularNumber::new_unchecked(
                vec![1, 2, 3, 4, 5, 6, 7, 8],
                EncodedModulo::U64SafePrime,
            )),
        )]);
        let program = PROGRAMS.program("simple_shares").unwrap().0;
        let Err(e) = builder.build_execution_vm(program, values, Default::default(), Uuid::new_v4()) else {
            panic!("not an error");
        };
        assert!(matches!(e, BuildExecutionVmError::InputValidation(_)), "not the right error type: {e:?}");
    }
}
