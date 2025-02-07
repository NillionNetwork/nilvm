//! The program auditor
#![deny(missing_docs)]
#![forbid(unsafe_code)]
#![deny(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::iterator_step_by_zero,
    clippy::invalid_regex,
    clippy::string_slice,
    clippy::unimplemented,
    clippy::todo
)]
#![cfg_attr(test, feature(box_patterns))]
#![feature(never_type)]

use std::{collections::HashMap, fmt::Display};

use mpc_vm::{
    requirements::{MPCProgramRequirements, ProgramRequirements},
    JitCompiler, JitCompilerError, MPCCompiler, Program, Protocol,
};
use nada_compiler_backend::{
    mir::{named_element, proto::ConvertProto, NamedElement, ProgramMIR},
    validators::Validator,
};
use thiserror::Error;

/// Program Auditor configuration.
#[derive(Clone, Debug, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ProgramAuditorConfig {
    /// Maximum amount of memory elements that are allowed.
    pub max_memory_size: u64,
    /// Maximum allowed total number of instructions.
    pub max_instructions: u64,
    /// Maximum allowed number of instructions per instruction type
    #[cfg_attr(feature = "serde", serde(default))]
    pub max_instructions_per_type: HashMap<String, u64>,
    /// Maximum amount of pre-processing elements that are allowed.
    pub max_preprocessing: MPCProgramRequirements,
    /// Disables the program auditor
    #[cfg_attr(feature = "serde", serde(skip))]
    pub disable: bool,
}

/// Program Auditor Request
///
/// Represents a request to the Program Auditor.  
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ProgramAuditorRequest {
    /// The program memory size
    pub memory_size: u64,
    /// The total number of instructions
    pub total_instructions: u64,
    /// The program instructions
    pub instructions: HashMap<String, u64>,
    /// The program preprocessing requirements
    pub preprocessing_requirements: MPCProgramRequirements,
}

impl ProgramAuditorRequest {
    /// Generates a new program auditor request from MIR.
    ///
    /// Runs validation, compiles the program and calculates the corresponding request.
    pub fn from_mir(mir: &ProgramMIR) -> Result<Self, ProgramAuditorError> {
        let validation_result =
            mir.validate().map_err(|e| ProgramAuditorError::Unexpected(format!("error during MIR validation: {e}")))?;
        if !validation_result.is_successful() {
            Err(ProgramAuditorError::MIRInvalid(validation_result.into()))?;
        }
        let program = MPCCompiler::compile(mir.clone())?;
        let preprocessing_requirements = MPCProgramRequirements::from_program(&program)
            .map_err(|e| ProgramAuditorError::Unexpected(format!("error calculating pre-processing elements {e}")))?;

        Ok(Self {
            memory_size: Self::calculate_program_memory(&program)? as u64,
            total_instructions: program.body.protocols.len() as u64,
            instructions: Self::calculate_instructions_map(&program)?,
            preprocessing_requirements,
        })
    }

    /// Generates a new program auditor request from a raw MIR.
    ///
    /// Runs validation, compiles the program and calculates the corresponding request.
    pub fn from_raw_mir(mir: &[u8]) -> Result<Self, ProgramAuditorError> {
        let mir = ProgramMIR::try_decode(mir)
            .map_err(|e| ProgramAuditorError::Unexpected(format!("error while deserializing MIR {e}")))?;
        Self::from_mir(&mir)
    }

    fn calculate_program_memory<P: Protocol>(program: &Program<P>) -> Result<usize, ProgramAuditorError> {
        if program.body.protocols.is_empty() {
            return Err(ProgramAuditorError::Unexpected(
                "This program is insecure because it has 0 operations and therefore leaks the input".to_string(),
            ));
        }
        Ok(program.body.memory_size())
    }

    /// Sorts protocols into categories and count them
    fn calculate_instructions_map<P: Protocol>(
        program: &Program<P>,
    ) -> Result<HashMap<String, u64>, ProgramAuditorError> {
        let mut instruction_map: HashMap<String, u64> = HashMap::new();
        for protocol in program.body.protocols.values() {
            let protocol_name = protocol.name();
            let current_count = *instruction_map.get(protocol_name).unwrap_or(&0u64);
            instruction_map.insert(protocol_name.to_owned(), current_count.wrapping_add(1u64));
        }
        Ok(instruction_map)
    }
}

#[derive(PartialEq, Debug)]
/// The program auditor policies supported
pub enum Policy {
    /// Maximum allowed Memory policy
    MaxMemory(MaxMemoryPolicy),
    /// Maximum amount of Instructions policy
    MaxInstructions(MaxInstructionsPolicy),
    /// Maximum amount of preprocessing elements policy
    MaxPreprocessing(MaxPreprocessingPolicy),
}

named_element!(
    (MaxMemoryPolicy, "max_memory"),
    (MaxInstructionsPolicy, "max_instructions"),
    (MaxPreprocessingPolicy, "max_preprocessing_elements")
);

impl Policy {
    /// List of policies to be executed.
    ///
    /// The [`Policy::Compile`] policy is missing from the list as it is executed separately.
    fn policies() -> Vec<Policy> {
        use Policy::*;
        vec![
            MaxMemory(MaxMemoryPolicy {}),
            MaxInstructions(MaxInstructionsPolicy {}),
            MaxPreprocessing(MaxPreprocessingPolicy {}),
        ]
    }
}

#[derive(Debug, PartialEq)]
/// Policy Violation representation
pub struct PolicyViolation {
    /// The policy violated
    pub policy: String,
    /// An explanatory message
    pub message: String,
}

impl Display for PolicyViolation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "failure in policy {}: {}", self.policy, self.message)
    }
}

#[derive(Clone, Debug)]
/// The Program Auditor
pub struct ProgramAuditor {
    config: ProgramAuditorConfig,
}

impl ProgramAuditor {
    /// Constructs a new instance of [`ProgramAuditor`]
    ///
    /// # Arguments
    /// * `config` - The configuration for the auditor
    pub fn new(config: ProgramAuditorConfig) -> Self {
        Self { config }
    }

    /// Audits a [`ProgramMIR`].
    ///
    /// The audit runs all the policies specified in the [`Policy`] enum. Whenever if finds a failure,
    /// it stops and returns the violation. The reason for this behaviour is to prevent unnecessary execution
    /// when the program has been detected as invalid. This unnecessary execution of policies has
    /// computing costs and also it could potentially be introducing additional risks.  
    ///
    /// # Arguments
    /// * `request` - The [`ProgramAuditorRequest`] that will be audited.
    ///
    /// # Returns
    /// An instance of [`Result`], if the audit passed it returns empty. Othewise, if there is an error due to an unexpected situation
    /// or policy failure it will return the corresponding error in the `InvalidProgram` variant of [`ProgramAuditorError`].  
    pub fn audit(&self, request: &ProgramAuditorRequest) -> Result<(), ProgramAuditorError> {
        if self.config.disable {
            return Ok(());
        }
        let context = ProgramAuditorContext { config: &self.config, request };
        // Lets run the policies. We will return at the first failure.
        for policy in Policy::policies() {
            policy.run(&context)?;
        }
        Ok(())
    }
}

/// The Program Auditor context
///
/// This structure is shared with all the policies executed in the audit.
pub struct ProgramAuditorContext<'a> {
    config: &'a ProgramAuditorConfig,
    request: &'a ProgramAuditorRequest,
}

trait PolicyRunner {
    fn run(&self, context: &ProgramAuditorContext) -> Result<(), ProgramAuditorError>;
}

/// Implementation of Max memory policy
#[derive(PartialEq, Debug)]
pub struct MaxMemoryPolicy;

impl PolicyRunner for MaxMemoryPolicy {
    fn run(&self, context: &ProgramAuditorContext) -> Result<(), ProgramAuditorError> {
        if context.request.memory_size > context.config.max_memory_size {
            Err(ProgramAuditorError::InvalidProgram(PolicyViolation {
                policy: self.name().to_string(),
                message: format!(
                    "maximum memory limit exceeded for program, program memory is {}, maximum: {}",
                    context.request.memory_size, context.config.max_memory_size
                ),
            }))
        } else {
            Ok(())
        }
    }
}

/// Implementation of Max Instructions Policy
#[derive(PartialEq, Debug)]
pub struct MaxInstructionsPolicy;

impl PolicyRunner for MaxInstructionsPolicy {
    fn run(&self, context: &ProgramAuditorContext) -> Result<(), ProgramAuditorError> {
        if context.request.total_instructions > context.config.max_instructions {
            return Err(ProgramAuditorError::InvalidProgram(PolicyViolation {
                policy: self.name().to_string(),
                message: format!(
                    "maximum total amount of instructions exceeded for program, instructions: {}, maximum: {}",
                    context.request.total_instructions, context.config.max_instructions
                ),
            }));
        }
        for (instruction, count) in context.request.instructions.iter() {
            if let Some(max_count) = context.config.max_instructions_per_type.get(instruction) {
                if count > max_count {
                    return Err(ProgramAuditorError::InvalidProgram(PolicyViolation {
                        policy: format!("{}[{}]", self.name(), instruction),
                        message: format!(
                            "maximum amount exceeded for instruction: {}, actual: {}, maximum: {}",
                            instruction, count, max_count
                        ),
                    }));
                }
            }
        }

        Ok(())
    }
}

/// Implementation of Max Preprocessing Policy
#[derive(PartialEq, Debug)]
pub struct MaxPreprocessingPolicy;

impl PolicyRunner for MaxPreprocessingPolicy {
    fn run(&self, context: &ProgramAuditorContext) -> Result<(), ProgramAuditorError> {
        let program_requirements = context.request.preprocessing_requirements.clone();
        for (requirement, max_value) in context.config.max_preprocessing.clone() {
            if max_value < program_requirements.runtime_requirement(&requirement) {
                return Err(ProgramAuditorError::InvalidProgram(PolicyViolation {
                    policy: format!("{}[{:?}]", self.name(), requirement),
                    message: format!(
                        "preprocessing requirements exceeded for {requirement:?}, max: {max_value}, actual: {}",
                        program_requirements.runtime_requirement(&requirement)
                    ),
                }));
            }
        }
        Ok(())
    }
}

impl PolicyRunner for Policy {
    fn run(&self, context: &ProgramAuditorContext) -> Result<(), ProgramAuditorError> {
        use Policy::*;
        match self {
            MaxInstructions(o) => o.run(context),
            MaxMemory(o) => o.run(context),
            MaxPreprocessing(o) => o.run(context),
        }
    }
}

impl NamedElement for Policy {
    fn name(&self) -> &str {
        use Policy::*;
        match self {
            MaxInstructions(o) => o.name(),
            MaxMemory(o) => o.name(),
            MaxPreprocessing(o) => o.name(),
        }
    }
}

/// The program auditor error.
#[derive(Error, Debug)]
#[repr(u8)]
pub enum ProgramAuditorError {
    /// Invalid program
    #[error("invalid program: {0}")]
    InvalidProgram(PolicyViolation) = 0,

    /// Unexpected error
    #[error("unexpected: {0}")]
    Unexpected(String) = 1,

    /// Program Compilation error
    #[error("program compilation error")]
    Compile(#[from] JitCompilerError) = 2,

    /// Invalid program
    #[error("program MIR is not valid: {0:?}")]
    MIRInvalid(Vec<String>) = 3,
}

#[cfg(test)]
mod test;
