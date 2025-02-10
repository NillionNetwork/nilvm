//! MPC program simulator implementation
use crate::{
    protocols::MPCProtocol,
    requirements::{MPCProgramRequirements, RuntimeRequirementType},
    vm::plan::MPCRuntimePreprocessingElements,
};
use anyhow::Error;
use basic_types::PartyId;
use execution_engine_vm::{
    metrics::ExecutionMetricsConfig,
    simulator::SimulatableProgram,
    vm::instructions::{get_statistic_k, STATISTIC_KAPPA},
};
pub use execution_engine_vm::{
    metrics::{ExecutionMetrics, MetricsFormat},
    simulator::{
        inputs::{InputGenerator, StaticInputGeneratorBuilder},
        ProgramSimulator, SimulationParameters,
    },
};
use jit_compiler::{requirements::ProgramRequirements, Program};
use math_lib::{
    impl_boxed_from_encoded_safe_prime,
    modular::{ModularNumber, SafePrime},
};
use nada_value::{clear::Clear, NadaValue};
use protocols::{
    conditionals::{
        equality::offline::{
            output::PrepPrivateOutputEqualityShares, validation::PrepPrivateOutputEqualitySharesBuilder,
        },
        equality_public_output::offline::{
            validation::PrepPublicOutputEqualitySharesBuilder, PrepPublicOutputEqualityShares,
        },
        less_than::offline::{validation::PrepCompareSharesBuilder, PrepCompareShares},
    },
    division::{
        division_secret_divisor::offline::{
            validation::PrepDivisionIntegerSecretSharesBuilder, PrepDivisionIntegerSecretShares,
        },
        modulo2m_public_divisor::offline::{validation::PrepModulo2mSharesBuilder, PrepModulo2mShares},
        modulo_public_divisor::offline::{validation::PrepModuloSharesBuilder, PrepModuloShares},
        truncation_probabilistic::offline::{validation::PrepTruncPrSharesBuilder, PrepTruncPrShares},
    },
    random::{
        random_bit::{validation::RandomBooleanSharesBuilder, BitShare},
        random_integer::validation::RandomIntegerSharesBuilder,
    },
    threshold_ecdsa::auxiliary_information::fake::FakeEcdsaAuxInfo,
};
use shamir_sharing::secret_sharer::{SafePrimeSecretSharer, SecretSharerProperties, ShamirSecretSharer};
use std::{collections::HashMap, convert::Infallible, marker::PhantomData};

impl<T> SimulatableProgram<MPCProtocol, T> for Program<MPCProtocol>
where
    T: SafePrime,
    ShamirSecretSharer<T>: SafePrimeSecretSharer<T>,
{
    type Provider = MPCRuntimePreprocessingElements;

    // Only used for testing so expect should be used.
    #[allow(clippy::expect_used, clippy::expect_fun_call, clippy::unwrap_used)]
    fn build_requirements_provider(
        &self,
        sharer: &ShamirSecretSharer<T>,
    ) -> Result<HashMap<PartyId, Self::Provider>, Error> {
        let requirements = MPCProgramRequirements::from_program(self)?;

        let prep_compare = PrepCompareSharesBuilder::new(sharer, rand::thread_rng())?
            .build(requirements.runtime_requirement(&RuntimeRequirementType::Compare))?;

        let prep_division_integer_secret =
            PrepDivisionIntegerSecretSharesBuilder::new(sharer, get_statistic_k::<T>(), STATISTIC_KAPPA)?
                .build(requirements.runtime_requirement(&RuntimeRequirementType::DivisionIntegerSecret))?;

        let prep_modulo = PrepModuloSharesBuilder::new(sharer, get_statistic_k::<T>(), STATISTIC_KAPPA)?
            .build(requirements.runtime_requirement(&RuntimeRequirementType::Modulo))?;

        let prep_public_output_equality = PrepPublicOutputEqualitySharesBuilder::new(sharer, rand::thread_rng())?
            .build(requirements.runtime_requirement(&RuntimeRequirementType::PublicOutputEquality))?;

        let prep_truncpr = PrepTruncPrSharesBuilder::new(sharer, get_statistic_k::<T>(), STATISTIC_KAPPA)?
            .build(requirements.runtime_requirement(&RuntimeRequirementType::TruncPr))?;

        let prep_trunc = PrepModulo2mSharesBuilder::new(sharer, get_statistic_k::<T>(), STATISTIC_KAPPA)?
            .build(requirements.runtime_requirement(&RuntimeRequirementType::Trunc))?;

        let prep_equals_integer_secret = PrepPrivateOutputEqualitySharesBuilder::new(sharer, rand::thread_rng())?
            .build(requirements.runtime_requirement(&RuntimeRequirementType::EqualsIntegerSecret))?;

        let random_integer = RandomIntegerSharesBuilder::new(sharer)?
            .build(requirements.runtime_requirement(&RuntimeRequirementType::RandomInteger))?;

        let random_boolean = RandomBooleanSharesBuilder::new(sharer)?
            .build(requirements.runtime_requirement(&RuntimeRequirementType::RandomBoolean))?;
        let ecdsa_aux_info = match requirements.runtime_requirement(&RuntimeRequirementType::EcdsaAuxInfo) {
            0 => None,
            _ => {
                let output = FakeEcdsaAuxInfo::generate_ecdsa(sharer.party_count() as u16)?;
                Some(output.try_into_element()?)
            }
        };

        let mut elements = HashMap::with_capacity(prep_compare.len());
        for party_id in sharer.parties() {
            let compare: Result<Vec<_>, Infallible> =
                if requirements.runtime_requirement(&RuntimeRequirementType::Compare) > 0 {
                    prep_compare
                        .get(&party_id)
                        .expect(&format!("Failed to retrieve compare share for {}", party_id))
                        .iter()
                        .map(PrepCompareShares::encode)
                        .collect()
                } else {
                    Ok(vec![])
                };

            let division_integer_secret: Result<Vec<_>, Infallible> =
                if requirements.runtime_requirement(&RuntimeRequirementType::DivisionIntegerSecret) > 0 {
                    prep_division_integer_secret
                        .get(&party_id)
                        .expect(&format!("Failed to retrieve division share for {}", party_id))
                        .iter()
                        .map(PrepDivisionIntegerSecretShares::encode)
                        .collect()
                } else {
                    Ok(vec![])
                };

            let modulo: Result<Vec<_>, Infallible> =
                if requirements.runtime_requirement(&RuntimeRequirementType::Modulo) > 0 {
                    prep_modulo
                        .get(&party_id)
                        .expect(&format!("Failed to retrieve modulo share for {}", party_id))
                        .iter()
                        .map(PrepModuloShares::encode)
                        .collect()
                } else {
                    Ok(vec![])
                };

            let public_output_equality: Result<Vec<_>, Infallible> =
                if requirements.runtime_requirement(&RuntimeRequirementType::PublicOutputEquality) > 0 {
                    prep_public_output_equality
                        .get(&party_id)
                        .expect(&format!("Failed to retrieve public output equals share for {}", party_id))
                        .iter()
                        .map(PrepPublicOutputEqualityShares::encode)
                        .collect()
                } else {
                    Ok(vec![])
                };

            let truncpr = if requirements.runtime_requirement(&RuntimeRequirementType::TruncPr) > 0 {
                prep_truncpr
                    .get(&party_id)
                    .expect(&format!("Failed to retrieve truncpr share for {}", party_id))
                    .iter()
                    .map(PrepTruncPrShares::encode)
                    .collect()
            } else {
                Ok(vec![])
            };

            let trunc = if requirements.runtime_requirement(&RuntimeRequirementType::Trunc) > 0 {
                prep_trunc
                    .get(&party_id)
                    .expect(&format!("Failed to retrieve trunc share for {}", party_id))
                    .iter()
                    .map(PrepModulo2mShares::encode)
                    .collect()
            } else {
                Ok(vec![])
            };

            let equals_integer_secret: Result<Vec<_>, Infallible> =
                if requirements.runtime_requirement(&RuntimeRequirementType::EqualsIntegerSecret) > 0 {
                    prep_equals_integer_secret
                        .get(&party_id)
                        .expect(&format!("Failed to retrieve public output equals share for {}", party_id))
                        .iter()
                        .map(PrepPrivateOutputEqualityShares::encode)
                        .collect()
                } else {
                    Ok(vec![])
                };

            let random_integer: Result<Vec<_>, Infallible> =
                if requirements.runtime_requirement(&RuntimeRequirementType::RandomInteger) > 0 {
                    Ok(random_integer
                        .get(&party_id)
                        .expect(&format!("Failed to retrieve random integer share for {}", party_id))
                        .iter()
                        .map(ModularNumber::encode)
                        .collect())
                } else {
                    Ok(vec![])
                };

            let random_boolean: Result<Vec<_>, Infallible> =
                if requirements.runtime_requirement(&RuntimeRequirementType::RandomBoolean) > 0 {
                    Ok(random_boolean
                        .get(&party_id)
                        .expect(&format!("Failed to retrieve random boolean share for {}", party_id))
                        .iter()
                        .map(BitShare::encode)
                        .collect())
                } else {
                    Ok(vec![])
                };

            elements.insert(
                party_id,
                MPCRuntimePreprocessingElements {
                    compare: compare?,
                    division_integer_secret: division_integer_secret?,
                    modulo: modulo?,
                    public_output_equality: public_output_equality?,
                    truncpr: truncpr?,
                    trunc: trunc?,
                    equals_integer_secret: equals_integer_secret?,
                    random_integer: random_integer?,
                    random_boolean: random_boolean?,
                    ecdsa_aux_info: ecdsa_aux_info.clone(),
                },
            );
        }
        Ok(elements)
    }
}

/// Programs simulator to use with SimulatorRunner in a encoded way getting rid of generics
#[derive(Default)]
pub struct MPCPrimeSimulatorRunner<T>
where
    T: SafePrime,
    ShamirSecretSharer<T>: SafePrimeSecretSharer<T>,
{
    _panthom: PhantomData<(T, ShamirSecretSharer<T>)>,
}

/// Programs simulator trait to encode it getting rid of generics
pub trait SimulatorRunner {
    /// run the program
    fn run(
        &self,
        program: Program<MPCProtocol>,
        parameters: SimulationParameters,
        secret_generator: &InputGenerator,
        metrics_options: ExecutionMetricsConfig,
    ) -> Result<(HashMap<String, NadaValue<Clear>>, ExecutionMetrics), Error>;
}

impl<T: SafePrime> SimulatorRunner for MPCPrimeSimulatorRunner<T>
where
    ShamirSecretSharer<T>: SafePrimeSecretSharer<T>,
{
    fn run(
        &self,
        program: Program<MPCProtocol>,
        parameters: SimulationParameters,
        input_generator: &InputGenerator,
        metrics_options: ExecutionMetricsConfig,
    ) -> Result<(HashMap<String, NadaValue<Clear>>, ExecutionMetrics), Error> {
        let simulator = ProgramSimulator::<MPCProtocol, T>::new(program, parameters, input_generator, metrics_options)?;
        simulator.run()
    }
}

impl_boxed_from_encoded_safe_prime!(MPCPrimeSimulatorRunner, SimulatorRunner);
