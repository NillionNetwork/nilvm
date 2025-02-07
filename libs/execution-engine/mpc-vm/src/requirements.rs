//! Requirements analysis.

use crate::protocols::MPCProtocol;
use anyhow::{anyhow, Error};
pub use jit_compiler::requirements::ProgramRequirements;
use jit_compiler::{models::protocols::Protocol, Program};
use std::collections::HashMap;
use strum::Display;

/// The runtime requirement types
#[derive(Debug, PartialEq, Eq, Ord, PartialOrd, Hash, Copy, Clone, Display)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum RuntimeRequirementType {
    /// the type for COMPARE Elements
    Compare,
    /// the type for DIVISION Elements
    DivisionIntegerSecret,
    /// the type for EQUALS Elements
    EqualsIntegerSecret,
    /// The type for MODULO elements
    Modulo,
    /// The type for PUBLIC-OUTPUT-EQUALITY elements
    PublicOutputEquality,
    /// The type for TRUNCPR elements
    TruncPr,
    /// The type for deterministic TRUNC elements
    Trunc,
    /// The type for RandomInteger elements
    RandomInteger,
    /// The type for RandomBoolean elements
    RandomBoolean,
    /// The ECDSA auxiliary information.
    EcdsaAuxInfo,
}

/// The pre-processing elements requirements program.
#[derive(Clone, Default, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct MPCProgramRequirements {
    /// The map of runtime elements
    runtime_elements: HashMap<RuntimeRequirementType, usize>,
}

impl ProgramRequirements<MPCProtocol> for MPCProgramRequirements {
    fn from_program(program: &Program<MPCProtocol>) -> Result<Self, Error> {
        // Calculate runtime requirements
        let requirements = program
            .body
            .protocols
            .values()
            .map(|p| MPCProgramRequirements::from_iter(p.runtime_requirements().iter().cloned()));
        MPCProgramRequirements::combine_all(requirements)
    }

    fn with_runtime_requirements(mut self, element_type: RuntimeRequirementType, count: usize) -> Self {
        self.runtime_elements.insert(element_type, count);
        self
    }

    fn runtime_requirement(&self, element_type: &RuntimeRequirementType) -> usize {
        self.runtime_elements.get(element_type).copied().unwrap_or_default()
    }
}

impl MPCProgramRequirements {
    /// Return the ProgramRequirements instance with the selected compare elements
    pub fn with_compare_elements(self, elements: usize) -> Self {
        self.with_runtime_requirements(RuntimeRequirementType::Compare, elements)
    }

    /// Return the ProgramRequirements instance with the selected division elements
    pub fn with_division_integer_secret_elements(self, elements: usize) -> Self {
        self.with_runtime_requirements(RuntimeRequirementType::DivisionIntegerSecret, elements)
    }

    /// Return the ProgramRequirements instance with the selected division elements
    pub fn with_equals_integer_secret_elements(self, elements: usize) -> Self {
        self.with_runtime_requirements(RuntimeRequirementType::EqualsIntegerSecret, elements)
    }

    /// Return the ProgramRequirements instance with the selected modulo elements
    pub fn with_modulo_elements(self, elements: usize) -> Self {
        self.with_runtime_requirements(RuntimeRequirementType::Modulo, elements)
    }

    /// Return the ProgramRequirements instance with the selected truncation elements
    pub fn with_trunc_elements(self, elements: usize) -> Self {
        self.with_runtime_requirements(RuntimeRequirementType::Trunc, elements)
    }

    /// Return the ProgramRequirements instance with the selected probabilistic truncation elements
    pub fn with_truncpr_elements(self, elements: usize) -> Self {
        self.with_runtime_requirements(RuntimeRequirementType::TruncPr, elements)
    }

    /// Return the ProgramRequirements instance with the selected public output equals elements
    pub fn with_public_output_equality_elements(self, elements: usize) -> Self {
        self.with_runtime_requirements(RuntimeRequirementType::PublicOutputEquality, elements)
    }

    /// Return the ProgramRequirements instance with the selected random integer elements
    pub fn with_random_integer_elements(self, elements: usize) -> Self {
        self.with_runtime_requirements(RuntimeRequirementType::RandomInteger, elements)
    }

    /// Return the ProgramRequirements instance with the selected random boolean elements
    pub fn with_random_boolean_elements(self, elements: usize) -> Self {
        self.with_runtime_requirements(RuntimeRequirementType::RandomBoolean, elements)
    }

    /// Return the ProgramRequirements instance expecting an ecdsa auxiliary material.
    pub fn with_ecdsa_aux_info(self) -> Self {
        self.with_runtime_requirements(RuntimeRequirementType::EcdsaAuxInfo, 1)
    }

    /// Return the number of required runtime elements
    pub fn runtime_elements(&self) -> &HashMap<RuntimeRequirementType, usize> {
        &self.runtime_elements
    }

    /// Combine all requirements into one.
    ///
    /// Given a list of requirements, it combines them, returning an instance of
    /// [`MPCProgramRequirements`] where each element is the sum of the elements for each category.
    pub fn combine_all(
        all_requirements: impl Iterator<Item = MPCProgramRequirements>,
    ) -> Result<MPCProgramRequirements, Error> {
        let mut combined = MPCProgramRequirements::default();
        for requirements in all_requirements {
            for (element_type, elements) in requirements.runtime_elements {
                combined.runtime_elements.insert(
                    element_type,
                    combined
                        .runtime_requirement(&element_type)
                        .checked_add(elements)
                        .ok_or_else(|| anyhow!("{:?} elements overflow", element_type))?,
                );
            }
        }
        Ok(combined)
    }
}

impl FromIterator<(RuntimeRequirementType, usize)> for MPCProgramRequirements {
    fn from_iter<T: IntoIterator<Item = (RuntimeRequirementType, usize)>>(iter: T) -> Self {
        let mut requirements = MPCProgramRequirements::default();
        for (element_type, amount) in iter {
            requirements = requirements.with_runtime_requirements(element_type, amount);
        }
        requirements
    }
}

impl IntoIterator for MPCProgramRequirements {
    type Item = (RuntimeRequirementType, usize);
    type IntoIter = <HashMap<RuntimeRequirementType, usize> as IntoIterator>::IntoIter;

    fn into_iter(self) -> Self::IntoIter {
        self.runtime_elements.into_iter()
    }
}

#[cfg(test)]
mod test {
    use crate::{
        protocols::{
            addition::Addition,
            division::DivisionIntegerSecretDivisor,
            equals::{EqualsSecret, PublicOutputEquality},
            less_than::LessThanShares,
            modulo::ModuloIntegerSecretDividendPublicDivisor,
        },
        requirements::{MPCProgramRequirements, RuntimeRequirementType},
        MPCCompiler, MPCProtocol,
    };
    use anyhow::Error;
    use jit_compiler::{
        models::{
            memory::AddressType,
            protocols::{memory::ProtocolAddress, Protocol, ProtocolsModel},
            SourceRefIndex,
        },
        requirements::ProgramRequirements,
        JitCompiler, Program,
    };
    use nada_value::NadaType;
    use rstest::rstest;
    use test_programs::PROGRAMS;

    #[test]
    fn analyze_identity() {
        let protocols: Vec<MPCProtocol> = vec![
            Addition {
                address: Default::default(),
                left: Default::default(),
                right: Default::default(),
                ty: NadaType::ShamirShareInteger,
                source_ref_index: SourceRefIndex::default(),
            }
            .into(),
        ];
        let body = ProtocolsModel {
            protocols: protocols.into_iter().map(|p| (p.address(), p)).collect(),
            ..Default::default()
        };
        let program = Program { contract: Default::default(), body };
        let requirements = MPCProgramRequirements::from_program(&program).unwrap();
        assert_eq!(requirements.runtime_requirement(&RuntimeRequirementType::Compare), 0);
    }

    #[test]
    fn analyze_compare() {
        let protocols: Vec<MPCProtocol> = vec![
            LessThanShares {
                address: Default::default(),
                left: Default::default(),
                right: Default::default(),
                ty: NadaType::ShamirShareBoolean,
                source_ref_index: SourceRefIndex::default(),
            }
            .into(),
        ];
        let body = ProtocolsModel {
            protocols: protocols.into_iter().map(|p| (p.address(), p)).collect(),
            ..Default::default()
        };
        let program = Program { contract: Default::default(), body };
        let requirements = MPCProgramRequirements::from_program(&program).unwrap();
        assert_eq!(requirements.runtime_requirement(&RuntimeRequirementType::Compare), 1);
    }

    #[test]
    fn analyze_multiple() {
        let protocols: Vec<MPCProtocol> = vec![
            LessThanShares {
                address: ProtocolAddress::new(0, AddressType::Heap),
                left: Default::default(),
                right: Default::default(),
                ty: NadaType::ShamirShareBoolean,
                source_ref_index: SourceRefIndex::default(),
            }
            .into(),
            ModuloIntegerSecretDividendPublicDivisor {
                address: ProtocolAddress::new(1, AddressType::Heap),
                left: Default::default(),
                right: Default::default(),
                ty: NadaType::ShamirShareInteger,
                source_ref_index: SourceRefIndex::default(),
            }
            .into(),
            ModuloIntegerSecretDividendPublicDivisor {
                address: ProtocolAddress::new(2, AddressType::Heap),
                left: Default::default(),
                right: Default::default(),
                ty: NadaType::ShamirShareInteger,
                source_ref_index: SourceRefIndex::default(),
            }
            .into(),
        ];
        let body = ProtocolsModel {
            protocols: protocols.into_iter().map(|p| (p.address(), p)).collect(),
            ..Default::default()
        };
        let program = Program { contract: Default::default(), body };
        let requirements = MPCProgramRequirements::from_program(&program).unwrap();
        assert_eq!(requirements.runtime_requirement(&RuntimeRequirementType::Compare), 1);
        assert_eq!(requirements.runtime_requirement(&RuntimeRequirementType::Modulo), 2);
    }

    #[test]
    fn analyze_division() {
        let protocols: Vec<MPCProtocol> = vec![
            DivisionIntegerSecretDivisor {
                address: Default::default(),
                left: Default::default(),
                right: Default::default(),
                ty: NadaType::ShamirShareInteger,
                source_ref_index: SourceRefIndex::default(),
            }
            .into(),
        ];
        let body = ProtocolsModel {
            protocols: protocols.into_iter().map(|p| (p.address(), p)).collect(),
            ..Default::default()
        };
        let program = Program { contract: Default::default(), body };
        let requirements = MPCProgramRequirements::from_program(&program).unwrap();
        assert_eq!(requirements.runtime_requirement(&RuntimeRequirementType::DivisionIntegerSecret), 1);
    }

    #[test]
    fn analyze_modulo() {
        let protocols: Vec<MPCProtocol> = vec![
            ModuloIntegerSecretDividendPublicDivisor {
                address: Default::default(),
                left: Default::default(),
                right: Default::default(),
                ty: NadaType::ShamirShareInteger,
                source_ref_index: SourceRefIndex::default(),
            }
            .into(),
        ];
        let body = ProtocolsModel {
            protocols: protocols.into_iter().map(|p| (p.address(), p)).collect(),
            ..Default::default()
        };
        let program = Program { contract: Default::default(), body };
        let requirements = MPCProgramRequirements::from_program(&program).unwrap();
        assert_eq!(requirements.runtime_requirement(&RuntimeRequirementType::Modulo), 1);
    }

    #[test]
    fn analyze_public_output_equality() {
        let protocols: Vec<MPCProtocol> = vec![
            PublicOutputEquality {
                address: Default::default(),
                left: Default::default(),
                right: Default::default(),
                ty: NadaType::Boolean,
                source_ref_index: SourceRefIndex::default(),
            }
            .into(),
        ];
        let body = ProtocolsModel {
            protocols: protocols.into_iter().map(|p| (p.address(), p)).collect(),
            ..Default::default()
        };
        let program = Program { contract: Default::default(), body };
        let requirements = MPCProgramRequirements::from_program(&program).unwrap();
        assert_eq!(requirements.runtime_requirement(&RuntimeRequirementType::PublicOutputEquality), 1);
    }

    #[test]
    fn analyze_equals_integer_secret() {
        let protocols: Vec<MPCProtocol> = vec![
            EqualsSecret {
                address: Default::default(),
                left: Default::default(),
                right: Default::default(),
                ty: NadaType::ShamirShareInteger,
                source_ref_index: SourceRefIndex::default(),
            }
            .into(),
        ];
        let body = ProtocolsModel {
            protocols: protocols.into_iter().map(|p| (p.address(), p)).collect(),
            ..Default::default()
        };
        let program = Program { contract: Default::default(), body };
        let requirements = MPCProgramRequirements::from_program(&program).unwrap();
        assert_eq!(requirements.runtime_requirement(&RuntimeRequirementType::EqualsIntegerSecret), 1);
    }

    #[rstest]
    #[case("big_recursion", MPCProgramRequirements::default())]
    #[case("greater_equal_mul", MPCProgramRequirements::default().with_compare_elements(1))]
    #[case("invalid_program", MPCProgramRequirements::default().with_division_integer_secret_elements(1001))]
    fn analyze_program(
        #[case] program_name: &str,
        #[case] expected_requirements: MPCProgramRequirements,
    ) -> Result<(), Error> {
        let program = MPCCompiler::compile(PROGRAMS.mir(program_name)?)?;
        let requirements = MPCProgramRequirements::from_program(&program)?;
        assert_eq!(expected_requirements, requirements);
        Ok(())
    }
}
