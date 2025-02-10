//! Just in time compiler implementation
#![forbid(unsafe_code)]
#![deny(
    missing_docs,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unreachable,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::iterator_step_by_zero,
    clippy::invalid_regex,
    clippy::string_slice,
    clippy::unimplemented,
    clippy::todo
)]
#![allow(clippy::module_inception)]

pub mod bytecode2protocol;
pub mod mir2bytecode;
pub mod models;
pub mod requirements;

use crate::{
    bytecode2protocol::errors::Bytecode2ProtocolError,
    mir2bytecode::errors::MIR2BytecodeError,
    models::protocols::{Protocol, ProtocolsModel},
};
use models::bytecode::ProgramBytecode;
use nada_compiler_backend::{
    mir::ProgramMIR,
    preprocess::error::MIRPreprocessorError,
    program_contract::{ProgramContract, ProgramContractError},
};

/// A program ready to be executed.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Program<P: Protocol> {
    /// This program's contract
    pub contract: ProgramContract,

    /// The body of this program.
    pub body: ProtocolsModel<P>,
}

/// The Jit compiler
pub trait JitCompiler<P: Protocol> {
    /// Compiles a program from its mir representation.
    fn compile(program: ProgramMIR) -> Result<Program<P>, JitCompilerError>;

    /// Compile with bytecode.
    ///
    /// Compiles a program from its MIR representation. Returns the [`ProgramBytecode`] in addition to the [`Program`].
    ///
    /// # Arguments
    /// * `program` - The [`ProgramMIR`] to be compiled.
    ///
    /// # Returns
    /// A tuple with the compiled [`Program`] and [`ProgramBytecode`] corresponding to the MIR.
    fn compile_with_bytecode(program: ProgramMIR) -> Result<(Program<P>, ProgramBytecode), JitCompilerError>;
}

/// An error during the jit compiler execution
#[derive(Debug, thiserror::Error)]
pub enum JitCompilerError {
    /// Program Contract building failed
    #[error("program contract building failed: {0}")]
    ProgramContractBuild(#[from] ProgramContractError),

    /// MIR preprocessor failed
    #[error("mir preprocessor failed: {0}")]
    MIRPreprocessorFailed(#[from] MIRPreprocessorError),

    /// MIR to Bytecode transformation failed
    #[error("bytecode compilation failed: {0}")]
    BytecodeTransformationFailed(#[from] MIR2BytecodeError),

    /// Program Contract building failed
    #[error("protocol compilation failed: {0}")]
    Bytecode2Protocol(#[from] Bytecode2ProtocolError),
}
