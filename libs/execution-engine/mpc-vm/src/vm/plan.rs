//! Implementation for MPC of the Protocol Preprocessing Elements

use crate::{protocols::MPCProtocol, requirements::RuntimeRequirementType};
use execution_engine_vm::vm::plan::InstructionRequirementProvider;
pub use execution_engine_vm::vm::plan::{ExecutionPlan, PlanCreateError};
use jit_compiler::models::protocols::Protocol;
use math_lib::modular::{EncodedModularNumber, ModularNumber, SafePrime};
use protocols::{
    conditionals::{
        equality::offline::{output::PrepPrivateOutputEqualityShares, EncodedPrepPrivateOutputEqualityShares},
        equality_public_output::offline::{EncodedPrepPublicOutputEqualityShares, PrepPublicOutputEqualityShares},
        less_than::offline::{EncodedPrepCompareShares, PrepCompareShares},
    },
    division::{
        division_secret_divisor::offline::{EncodedPrepDivisionIntegerSecretShares, PrepDivisionIntegerSecretShares},
        modulo2m_public_divisor::offline::{EncodedPrepModulo2mShares, PrepModulo2mShares},
        modulo_public_divisor::offline::{EncodedPrepModuloShares, PrepModuloShares},
        truncation_probabilistic::offline::{EncodedPrepTruncPrShares, PrepTruncPrShares},
    },
    random::random_bit::{BitShare, EncodedBitShare},
    threshold_ecdsa::auxiliary_information::output::EcdsaAuxInfo,
};
use shamir_sharing::secret_sharer::{SafePrimeSecretSharer, ShamirSecretSharer};

#[derive(Debug)]
/// Implements the collection of preprocessing elements that the MPC protocols know.
pub struct MPCProtocolPreprocessingElements<T: SafePrime> {
    pub(crate) compare: Vec<PrepCompareShares<T>>,
    pub(crate) division_integer_secret: Vec<PrepDivisionIntegerSecretShares<T>>,
    pub(crate) modulo: Vec<PrepModuloShares<T>>,
    pub(crate) public_output_equality: Vec<PrepPublicOutputEqualityShares<T>>,
    pub(crate) trunc: Vec<PrepModulo2mShares<T>>,
    pub(crate) truncpr: Vec<PrepTruncPrShares<T>>,
    pub(crate) equals_integer_secret: Vec<PrepPrivateOutputEqualityShares<T>>,
    pub(crate) random_integer: Vec<ModularNumber<T>>,
    pub(crate) random_boolean: Vec<BitShare<T>>,
    pub(crate) ecdsa_aux_info: Option<EcdsaAuxInfo>,
}

impl<T: SafePrime> Default for MPCProtocolPreprocessingElements<T> {
    fn default() -> Self {
        MPCProtocolPreprocessingElements {
            compare: Vec::new(),
            division_integer_secret: Vec::new(),
            modulo: Vec::new(),
            public_output_equality: Vec::new(),
            trunc: Vec::new(),
            truncpr: Vec::new(),
            equals_integer_secret: Vec::new(),
            random_integer: Vec::new(),
            random_boolean: Vec::new(),
            ecdsa_aux_info: None,
        }
    }
}

impl<T: SafePrime> MPCProtocolPreprocessingElements<T> {
    /// Update the `compare` preprocessing elements that the protocol needs
    pub fn with_compare(mut self, compare: Vec<PrepCompareShares<T>>) -> MPCProtocolPreprocessingElements<T> {
        self.compare = compare;
        self
    }

    /// Update the `division` preprocessing elements that the protocol needs
    pub fn with_division_integer_secret(
        mut self,
        division: Vec<PrepDivisionIntegerSecretShares<T>>,
    ) -> MPCProtocolPreprocessingElements<T> {
        self.division_integer_secret = division;
        self
    }

    /// Update the `equals-integer-secret` preprocessing elements that the protocol needs
    pub fn with_equals_integer_secret(
        mut self,
        equals: Vec<PrepPrivateOutputEqualityShares<T>>,
    ) -> MPCProtocolPreprocessingElements<T> {
        self.equals_integer_secret = equals;
        self
    }

    /// Update the `modulo` preprocessing elements that the protocol needs
    pub fn with_modulo(mut self, modulo: Vec<PrepModuloShares<T>>) -> MPCProtocolPreprocessingElements<T> {
        self.modulo = modulo;
        self
    }

    /// Update the `public-output-equality` preprocessing elements that the protocol needs
    pub fn with_public_output_equality(
        mut self,
        public_output_equality: Vec<PrepPublicOutputEqualityShares<T>>,
    ) -> MPCProtocolPreprocessingElements<T> {
        self.public_output_equality = public_output_equality;
        self
    }

    /// Update the `trunc` preprocessing elements that the protocol needs
    pub fn with_trunc(mut self, trunc: Vec<PrepModulo2mShares<T>>) -> MPCProtocolPreprocessingElements<T> {
        self.trunc = trunc;
        self
    }

    /// Update the `truncpr` preprocessing elements that the protocol needs
    pub fn with_truncpr(mut self, truncpr: Vec<PrepTruncPrShares<T>>) -> MPCProtocolPreprocessingElements<T> {
        self.truncpr = truncpr;
        self
    }

    /// Update the `random-integer` preprocessing elements that the protocol needs
    pub fn with_random_integer(mut self, random_integer: Vec<ModularNumber<T>>) -> MPCProtocolPreprocessingElements<T> {
        self.random_integer = random_integer;
        self
    }

    /// Update the `random-boolean` preprocessing elements that the protocol needs
    pub fn with_random_boolean(mut self, random_boolean: Vec<BitShare<T>>) -> MPCProtocolPreprocessingElements<T> {
        self.random_boolean = random_boolean;
        self
    }

    /// Update the ecdsa aux info that the protocol needs.
    pub fn with_ecdsa_aux_info(mut self, aux_info: EcdsaAuxInfo) -> MPCProtocolPreprocessingElements<T> {
        self.ecdsa_aux_info = Some(aux_info);
        self
    }
}

/// The runtime preprocessing elements to be used.
#[derive(Default)]
pub struct MPCRuntimePreprocessingElements {
    /// The COMPARE preprocessing elements.
    pub compare: Vec<EncodedPrepCompareShares>,
    /// The DIVISION preprocessing elements.
    pub division_integer_secret: Vec<EncodedPrepDivisionIntegerSecretShares>,
    /// The MODULO preprocessing elements.
    pub modulo: Vec<EncodedPrepModuloShares>,
    /// The PUBLIC-OUTPUT-EQUALITY preprocessing elements.
    pub public_output_equality: Vec<EncodedPrepPublicOutputEqualityShares>,
    /// The TRUNCPR preprocessing elements.
    pub truncpr: Vec<EncodedPrepTruncPrShares>,
    /// The TRUNC (MOD2M) preprocessing elements.
    pub trunc: Vec<EncodedPrepModulo2mShares>,
    /// The EQUALS preprocessing elements.
    pub equals_integer_secret: Vec<EncodedPrepPrivateOutputEqualityShares>,
    /// The RandomInteger preprocessing elements.
    pub random_integer: Vec<EncodedModularNumber>,
    /// The RandomBoolean preprocessing elements.
    pub random_boolean: Vec<EncodedBitShare>,
    /// The ECDSA auxiliary information.
    pub ecdsa_aux_info: Option<EcdsaAuxInfo>,
}

impl MPCRuntimePreprocessingElements {
    fn take_elements<P>(
        elements: &mut Vec<P>,
        amount: usize,
        elements_type: &'static str,
    ) -> Result<Vec<P>, PlanCreateError> {
        let mut protocol_elements = vec![];
        for _ in 0..amount {
            protocol_elements.push(elements.pop().ok_or(PlanCreateError::NotEnoughElements(elements_type))?)
        }
        Ok(protocol_elements)
    }
}

impl<T> InstructionRequirementProvider<T> for MPCRuntimePreprocessingElements
where
    T: SafePrime,
    ShamirSecretSharer<T>: SafePrimeSecretSharer<T>,
{
    type PreprocessingElement = MPCProtocolPreprocessingElements<T>;
    type Instruction = MPCProtocol;

    fn take(&mut self, protocol: &Self::Instruction) -> Result<MPCProtocolPreprocessingElements<T>, PlanCreateError> {
        let mut preprocessing_elements = MPCProtocolPreprocessingElements::default();
        for (requirement, amount) in protocol.runtime_requirements() {
            match requirement {
                RuntimeRequirementType::Compare => {
                    let shares = Self::take_elements(&mut self.compare, *amount, "COMPARE")?;
                    let decoded_shares = shares.into_iter().map(|s| s.try_decode()).collect::<Result<Vec<_>, _>>()?;
                    preprocessing_elements = preprocessing_elements.with_compare(decoded_shares);
                }
                RuntimeRequirementType::DivisionIntegerSecret => {
                    let shares = Self::take_elements(&mut self.division_integer_secret, *amount, "DIVISION")?;
                    let decoded_shares = shares.into_iter().map(|s| s.try_decode()).collect::<Result<Vec<_>, _>>()?;
                    preprocessing_elements = preprocessing_elements.with_division_integer_secret(decoded_shares);
                }
                RuntimeRequirementType::EqualsIntegerSecret => {
                    let shares = Self::take_elements(&mut self.equals_integer_secret, *amount, "EQUALS")?;
                    let decoded_shares = shares.into_iter().map(|s| s.try_decode()).collect::<Result<Vec<_>, _>>()?;
                    preprocessing_elements = preprocessing_elements.with_equals_integer_secret(decoded_shares);
                }
                RuntimeRequirementType::Modulo => {
                    let shares = Self::take_elements(&mut self.modulo, *amount, "MODULO")?;
                    let decoded_shares = shares.into_iter().map(|s| s.try_decode()).collect::<Result<Vec<_>, _>>()?;
                    preprocessing_elements = preprocessing_elements.with_modulo(decoded_shares);
                }
                RuntimeRequirementType::PublicOutputEquality => {
                    let shares =
                        Self::take_elements(&mut self.public_output_equality, *amount, "PUBLIC-OUTPUT-EQUALITY")?;
                    let decoded_shares = shares.into_iter().map(|s| s.try_decode()).collect::<Result<Vec<_>, _>>()?;
                    preprocessing_elements = preprocessing_elements.with_public_output_equality(decoded_shares);
                }
                RuntimeRequirementType::TruncPr => {
                    let shares = Self::take_elements(&mut self.truncpr, *amount, "TRUNCPR")?;
                    let decoded_shares = shares.into_iter().map(|s| s.try_decode()).collect::<Result<Vec<_>, _>>()?;
                    preprocessing_elements = preprocessing_elements.with_truncpr(decoded_shares);
                }
                RuntimeRequirementType::Trunc => {
                    let shares = Self::take_elements(&mut self.trunc, *amount, "TRUNC")?;
                    let decoded_shares = shares.into_iter().map(|s| s.try_decode()).collect::<Result<Vec<_>, _>>()?;
                    preprocessing_elements = preprocessing_elements.with_trunc(decoded_shares);
                }
                RuntimeRequirementType::RandomInteger => {
                    let shares = Self::take_elements(&mut self.random_integer, *amount, "RANDOM-INTEGER")?;
                    let decoded_shares = shares.into_iter().map(|s| s.try_decode()).collect::<Result<Vec<_>, _>>()?;
                    preprocessing_elements = preprocessing_elements.with_random_integer(decoded_shares);
                }
                RuntimeRequirementType::RandomBoolean => {
                    let shares = Self::take_elements(&mut self.random_boolean, *amount, "RANDOM-BOOLEAN")?;
                    let decoded_shares = shares.into_iter().map(|s| s.try_decode()).collect::<Result<Vec<_>, _>>()?;
                    preprocessing_elements = preprocessing_elements.with_random_boolean(decoded_shares);
                }
                RuntimeRequirementType::EcdsaAuxInfo => {
                    let aux_info =
                        self.ecdsa_aux_info.clone().ok_or(PlanCreateError::NotEnoughElements("ECDSA-AUX-INFO"))?;
                    preprocessing_elements.ecdsa_aux_info = Some(aux_info);
                }
            }
        }
        Ok(preprocessing_elements)
    }
}

#[cfg(test)]
mod tests {
    use crate::{MPCCompiler, MPCProtocol};
    use anyhow::Error;
    use execution_engine_vm::vm::plan::{parallel::parallel_plan, DummyProtocolRequirementProvider, ExecutionPlan};
    use jit_compiler::JitCompiler;
    use math_lib::modular::U64SafePrime;
    use test_programs::PROGRAMS;

    fn generate_parallel_plan(program_id: &'static str) -> Result<ExecutionPlan<MPCProtocol, U64SafePrime>, Error> {
        // We can not use PROGRAM.program(program_id) directly, because a cyclic dependency exists
        let mir = PROGRAMS.mir(program_id)?;
        let program = MPCCompiler::compile(mir)?;
        let plan = parallel_plan(program.body, DummyProtocolRequirementProvider::default())?;
        Ok(plan) //.map_err(|e| anyhow!("{e:?}"))
    }

    #[test]
    #[allow(clippy::indexing_slicing)]
    // result = a + (b / Integer(2))
    fn plan_addition_division() -> Result<(), Error> {
        let plan = generate_parallel_plan("addition_division")?;
        assert_eq!(plan.steps.len(), 2);
        assert_eq!(plan.steps[0].local.len(), 0);
        // b / Integer(2)
        assert_eq!(plan.steps[0].online.len(), 1);
        assert!(matches!(plan.steps[0].online[0].0, MPCProtocol::DivisionIntegerSecretDividendPublicDivisor(_)));
        assert_eq!(plan.steps[1].local.len(), 1);
        assert!(matches!(plan.steps[1].local[0], MPCProtocol::Addition(_)));
        assert_eq!(plan.steps[1].online.len(), 0);
        Ok(())
    }

    #[test]
    #[allow(clippy::indexing_slicing)]
    // result = a + (b / Integer(2))
    fn plan_addition_division_public() -> Result<(), Error> {
        let plan = generate_parallel_plan("addition_division_public")?;
        assert_eq!(plan.steps.len(), 1);
        // a + b / Integer(2)
        assert_eq!(plan.steps[0].local.len(), 2);
        assert_eq!(plan.steps[0].online.len(), 0);
        Ok(())
    }

    #[test]
    #[allow(clippy::indexing_slicing)]
    // a = SecretUnsignedInteger(Input(name="A", party=party1))
    // b = SecretUnsignedInteger(Input(name="B", party=party1))
    // c = PublicUnsignedInteger(Input(name="C", party=party1))
    // d = PublicUnsignedInteger(Input(name="D", party=party1))
    //
    // result = a * b + (b / UnsignedInteger(2)) + (c ** d) + (a % UnsignedInteger(5))
    fn plan_addition_mix_operations() -> Result<(), Error> {
        let plan = generate_parallel_plan("addition_mix_operations")?;
        assert_eq!(plan.steps.len(), 2);
        // c ** d
        assert_eq!(plan.steps[0].local.len(), 1);
        assert!(matches!(plan.steps[0].local[0], MPCProtocol::PowerPublicBasePublicExponent(_)));
        // a * b
        // b / UnsignedInteger(2)
        // a % UnsignedInteger(5)
        assert_eq!(plan.steps[0].online.len(), 3);
        assert!(matches!(plan.steps[0].online[0].0, MPCProtocol::MultiplicationShares(_)));
        assert!(matches!(plan.steps[0].online[1].0, MPCProtocol::DivisionIntegerSecretDividendPublicDivisor(_)));
        assert!(matches!(plan.steps[0].online[2].0, MPCProtocol::ModuloIntegerSecretDividendPublicDivisor(_)));
        // a * b + (b / UnsignedInteger(2)) + (c ** d) + (a % UnsignedInteger(5))
        assert_eq!(plan.steps[1].local.len(), 3);
        assert!(matches!(plan.steps[1].local[0], MPCProtocol::Addition(_)));
        assert!(matches!(plan.steps[1].local[1], MPCProtocol::Addition(_)));
        assert!(matches!(plan.steps[1].local[2], MPCProtocol::Addition(_)));
        assert_eq!(plan.steps[1].online.len(), 0);
        Ok(())
    }

    #[test]
    #[allow(clippy::indexing_slicing)]
    fn plan_big_recursion() -> Result<(), Error> {
        let plan = generate_parallel_plan("big_recursion")?;
        assert_eq!(plan.steps.len(), 1);
        assert_eq!(plan.steps[0].local.len(), 73);
        for i in 0..73 {
            assert!(matches!(plan.steps[0].local[i], MPCProtocol::Addition(_)));
        }
        assert_eq!(plan.steps[0].online.len(), 0);
        Ok(())
    }

    #[test]
    #[allow(clippy::indexing_slicing)]
    // TMP1 = A * B
    // PRODUCT1 = TMP1 * C
    // TMP2 = C * D
    // PRODUCT2 = TMP2 * E
    // PRODUCT3 = E * F
    // PARTIAL = PRODUCT1 + PRODUCT2
    // FINAL = PARTIAL + PRODUCT3
    fn plan_complex() -> Result<(), Error> {
        let plan = generate_parallel_plan("complex")?;
        assert_eq!(plan.steps.len(), 3);
        assert_eq!(plan.steps[0].local.len(), 0);
        assert_eq!(plan.steps[0].online.len(), 3);
        assert!(matches!(plan.steps[0].online[0].0, MPCProtocol::MultiplicationShares(_))); // TMP1 = A * B
        assert!(matches!(plan.steps[0].online[1].0, MPCProtocol::MultiplicationShares(_))); // TMP2 = C * D
        assert!(matches!(plan.steps[0].online[2].0, MPCProtocol::MultiplicationShares(_))); // PRODUCT3 = E * F
        assert_eq!(plan.steps[1].local.len(), 0);
        assert_eq!(plan.steps[1].online.len(), 2);
        assert!(matches!(plan.steps[1].online[0].0, MPCProtocol::MultiplicationShares(_))); // PRODUCT1 = TMP1 * C
        assert!(matches!(plan.steps[1].online[1].0, MPCProtocol::MultiplicationShares(_))); // PRODUCT2 = TMP2 * E
        assert_eq!(plan.steps[2].local.len(), 2);
        assert!(matches!(plan.steps[2].local[0], MPCProtocol::Addition(_))); // PARTIAL = PRODUCT1 + PRODUCT2
        assert!(matches!(plan.steps[2].local[1], MPCProtocol::Addition(_))); // FINAL = PARTIAL + PRODUCT3
        assert_eq!(plan.steps[2].online.len(), 0);
        Ok(())
    }

    #[test]
    #[allow(clippy::indexing_slicing)]
    // four_n0_n2_minus_n1_sq = Integer(4) * n0 * n2 - n1 * n1
    // alpha = four_n0_n2_minus_n1_sq * four_n0_n2_minus_n1_sq
    //
    // two_n0_plus_n1 = Integer(2) * n0 + n1
    // beta_1 = Integer(2) * two_n0_plus_n1 * two_n0_plus_n1
    //
    // two_n2_plus_n1 = Integer(2) * n2 + n1
    // beta_2 = two_n0_plus_n1 * two_n2_plus_n1
    //
    // beta_3 = Integer(2) * two_n2_plus_n1 * two_n2_plus_n1
    fn plan_chi_squared() -> Result<(), Error> {
        let plan = generate_parallel_plan("chi_squared")?;
        assert_eq!(plan.steps.len(), 2);
        assert_eq!(plan.steps[0].local.len(), 7);
        assert!(matches!(plan.steps[0].local[0], MPCProtocol::MultiplicationSharePublic(_))); // Integer(4) * n0
        assert!(matches!(plan.steps[0].local[1], MPCProtocol::MultiplicationSharePublic(_))); // Integer(2) * n0
        assert!(matches!(plan.steps[0].local[2], MPCProtocol::Addition(_))); // Integer(2) * n0 + n1
        assert!(matches!(plan.steps[0].local[3], MPCProtocol::MultiplicationSharePublic(_))); // Integer(2) * two_n0_plus_n1
        assert!(matches!(plan.steps[0].local[4], MPCProtocol::MultiplicationSharePublic(_))); // Integer(2) * n2
        assert!(matches!(plan.steps[0].local[5], MPCProtocol::Addition(_))); // Integer(2) * n2 + n1
        assert!(matches!(plan.steps[0].local[6], MPCProtocol::MultiplicationSharePublic(_))); // Integer(2) * two_n2_plus_n1
        assert_eq!(plan.steps[0].online.len(), 5);
        assert!(matches!(plan.steps[0].online[0].0, MPCProtocol::MultiplicationShares(_))); // Integer(4) * n0 * n2
        assert!(matches!(plan.steps[0].online[1].0, MPCProtocol::MultiplicationShares(_))); // n1 * n1
        assert!(matches!(plan.steps[0].online[2].0, MPCProtocol::MultiplicationShares(_))); // Integer(2) * two_n0_plus_n1 * two_n0_plus_n1
        assert!(matches!(plan.steps[0].online[3].0, MPCProtocol::MultiplicationShares(_))); // two_n0_plus_n1 * two_n2_plus_n1
        assert!(matches!(plan.steps[0].online[4].0, MPCProtocol::MultiplicationShares(_))); // Integer(2) * two_n2_plus_n1 * two_n2_plus_n1
        assert_eq!(plan.steps[1].local.len(), 1);
        assert!(matches!(plan.steps[1].local[0], MPCProtocol::Subtraction(_))); // Integer(4) * n0 * n2 - n1 * n1
        assert_eq!(plan.steps[1].online.len(), 1);
        assert!(matches!(plan.steps[1].online[0].0, MPCProtocol::MultiplicationShares(_))); // four_n0_n2_minus_n1_sq * four_n0_n2_minus_n1_sq
        Ok(())
    }

    #[test]
    #[allow(clippy::indexing_slicing)]
    fn plan_input_array() -> Result<(), Error> {
        let plan = generate_parallel_plan("input_array")?;
        assert_eq!(plan.steps.len(), 1);
        assert_eq!(plan.steps[0].local.len(), 1);
        assert!(matches!(plan.steps[0].local[0], MPCProtocol::NewArray(_)));
        assert_eq!(plan.steps[0].online.len(), 0);
        Ok(())
    }

    #[test]
    #[allow(clippy::indexing_slicing)]
    fn plan_inner_product() -> Result<(), Error> {
        let plan = generate_parallel_plan("inner_product")?;
        let array_size = 3usize;
        assert_eq!(plan.steps.len(), 2);
        assert_eq!(plan.steps[0].local.len(), 0);
        assert_eq!(plan.steps[0].online.len(), array_size);
        for i in 0..array_size {
            assert!(matches!(plan.steps[0].online[i].0, MPCProtocol::MultiplicationShares(_))); // c[i] = a[i] * b[i]
        }
        assert_eq!(plan.steps[1].local.len(), 3);
        for i in 0..array_size {
            assert!(matches!(plan.steps[1].local[i], MPCProtocol::Addition(_))); // accum = accum + c[i]
        }
        assert_eq!(plan.steps[1].online.len(), 0);
        Ok(())
    }

    #[test]
    #[allow(clippy::indexing_slicing)]
    fn plan_hamming_distance() -> Result<(), Error> {
        let plan = generate_parallel_plan("hamming_distance")?;
        let array_size = 3usize;
        assert_eq!(plan.steps.len(), 2);
        assert_eq!(plan.steps[0].local.len(), 0);
        assert_eq!(plan.steps[0].online.len(), array_size);
        for i in 0..array_size {
            assert!(matches!(plan.steps[0].online[i].0, MPCProtocol::EqualsSecret(_)));
        }
        assert_eq!(plan.steps[1].local.len(), array_size * 2);
        for i in 0..array_size {
            assert!(matches!(plan.steps[1].local[i], MPCProtocol::IfElsePublicBranches(_)));
        }
        for i in array_size..array_size * 2 {
            assert!(matches!(plan.steps[1].local[i], MPCProtocol::Addition(_)));
        }
        assert_eq!(plan.steps[1].online.len(), 0);
        Ok(())
    }

    #[test]
    #[allow(clippy::indexing_slicing)]
    fn plan_cardio_risk_factor() -> Result<(), Error> {
        let plan = generate_parallel_plan("cardio_risk_factor")?;
        assert_eq!(plan.steps.len(), 3);
        assert_eq!(plan.steps[0].local.len(), 1);
        assert!(matches!(plan.steps[0].local[0], MPCProtocol::Subtraction(_))); // height - UnsignedInteger(90)
        assert_eq!(plan.steps[0].online.len(), 11);
        assert!(matches!(plan.steps[0].online[0].0, MPCProtocol::LessThanShares(_))); // sex < UnsignedInteger(1)
        assert!(matches!(plan.steps[0].online[1].0, MPCProtocol::LessThanShares(_))); // age > UnsignedInteger(50)
        assert!(matches!(plan.steps[0].online[2].0, MPCProtocol::LessThanShares(_))); // sex > UnsignedInteger(0)
        assert!(matches!(plan.steps[0].online[3].0, MPCProtocol::LessThanShares(_))); // age > UnsignedInteger(60)
        assert!(matches!(plan.steps[0].online[4].0, MPCProtocol::LessThanShares(_))); // hdl_cholesterol < UnsignedInteger(40)
        assert!(matches!(plan.steps[0].online[5].0, MPCProtocol::LessThanShares(_))); // weight > (height - UnsignedInteger(90))
        assert!(matches!(plan.steps[0].online[6].0, MPCProtocol::LessThanShares(_))); // physical_act < UnsignedInteger(30)
        assert!(matches!(plan.steps[0].online[7].0, MPCProtocol::LessThanShares(_))); // sex < UnsignedInteger(1)
        assert!(matches!(plan.steps[0].online[8].0, MPCProtocol::LessThanShares(_))); // drinking > UnsignedInteger(3)
        assert!(matches!(plan.steps[0].online[9].0, MPCProtocol::LessThanShares(_))); // sex > UnsignedInteger(0)
        assert!(matches!(plan.steps[0].online[10].0, MPCProtocol::LessThanShares(_))); // drinking > UnsignedInteger(2)
        assert_eq!(plan.steps[1].local.len(), 7);
        // (age > UnsignedInteger(50)).if_else(UnsignedInteger(1), UnsignedInteger(0))
        assert!(matches!(plan.steps[1].local[0], MPCProtocol::IfElsePublicBranches(_)));
        // (age > UnsignedInteger(60)).if_else(UnsignedInteger(1), UnsignedInteger(0))
        assert!(matches!(plan.steps[1].local[1], MPCProtocol::IfElsePublicBranches(_)));
        // (hdl_cholesterol < UnsignedInteger(40)).if_else(UnsignedInteger(1), UnsignedInteger(0))
        assert!(matches!(plan.steps[1].local[2], MPCProtocol::IfElsePublicBranches(_)));
        // (weight > (height - UnsignedInteger(90))).if_else(UnsignedInteger(1), UnsignedInteger(0))
        assert!(matches!(plan.steps[1].local[3], MPCProtocol::IfElsePublicBranches(_)));
        // (physical_act < UnsignedInteger(30)).if_else(UnsignedInteger(1), UnsignedInteger(0))
        assert!(matches!(plan.steps[1].local[4], MPCProtocol::IfElsePublicBranches(_)));
        // (drinking > UnsignedInteger(3)).if_else(UnsignedInteger(1), UnsignedInteger(0))
        assert!(matches!(plan.steps[1].local[5], MPCProtocol::IfElsePublicBranches(_)));
        // (drinking > UnsignedInteger(2)).if_else(UnsignedInteger(1), UnsignedInteger(0))
        assert!(matches!(plan.steps[1].local[6], MPCProtocol::IfElsePublicBranches(_)));
        assert_eq!(plan.steps[1].online.len(), 4);
        // (sex < UnsignedInteger(1)).if_else(
        //      (age > UnsignedInteger(50)).if_else(UnsignedInteger(1), UnsignedInteger(0)),
        //      UnsignedInteger(0))
        assert!(matches!(plan.steps[1].online[0].0, MPCProtocol::IfElse(_)));
        // (sex > UnsignedInteger(0)).if_else(
        //      (age > UnsignedInteger(60)).if_else(UnsignedInteger(1), UnsignedInteger(0)),
        //      UnsignedInteger(0))
        assert!(matches!(plan.steps[1].online[1].0, MPCProtocol::IfElse(_)));
        // (sex < UnsignedInteger(1)).if_else(
        //      (drinking > UnsignedInteger(3)).if_else(UnsignedInteger(1), UnsignedInteger(0)),
        //      UnsignedInteger(0))
        assert!(matches!(plan.steps[1].online[2].0, MPCProtocol::IfElse(_)));
        // (sex > UnsignedInteger(0)).if_else(
        //      (drinking > UnsignedInteger(2)).if_else(UnsignedInteger(1), UnsignedInteger(0)),
        //      UnsignedInteger(0))
        assert!(matches!(plan.steps[1].online[3].0, MPCProtocol::IfElse(_)));
        assert_eq!(plan.steps[2].local.len(), 10);
        // Risk factor accum
        for i in 0..10 {
            assert!(matches!(plan.steps[2].local[i], MPCProtocol::Addition(_)));
        }
        assert_eq!(plan.steps[2].online.len(), 0);
        Ok(())
    }
}
