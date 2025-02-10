//! MPC protocol model implementation
//!
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
    clippy::unimplemented
)]
#![allow(clippy::module_inception)]

pub mod bytecode2protocol;
pub mod protocols;
pub mod requirements;
pub mod utils;
#[cfg(any(test, feature = "vm"))]
pub mod vm;

use crate::{bytecode2protocol::MPCProtocolFactory, protocols::MPCProtocol};
pub use jit_compiler::{
    bytecode2protocol::Bytecode2Protocol,
    mir2bytecode::MIR2Bytecode,
    models::{
        bytecode::{ProgramBytecode, BYTECODE_FILE_EXTENSION_BIN, BYTECODE_FILE_EXTENSION_JSON},
        memory::address_count,
        protocols::{Protocol, PROTOCOLS_BODY_FILE_EXTENSION_BIN, PROTOCOLS_BODY_FILE_EXTENSION_JSON},
    },
    JitCompiler, JitCompilerError, Program,
};
use nada_compiler_backend::{mir::ProgramMIR, program_contract::ProgramContract};

/// The JIT compiler
pub struct MPCCompiler;

impl JitCompiler<MPCProtocol> for MPCCompiler {
    fn compile(program: ProgramMIR) -> Result<Program<MPCProtocol>, JitCompilerError> {
        Ok(Self::compile_with_bytecode(program)?.0)
    }

    fn compile_with_bytecode(program: ProgramMIR) -> Result<(Program<MPCProtocol>, ProgramBytecode), JitCompilerError> {
        let contract = ProgramContract::from_program_mir(&program)?;
        let bytecode = MIR2Bytecode::transform(&program)?;
        let body = Bytecode2Protocol::transform(MPCProtocolFactory, &bytecode)?;
        Ok((Program { contract, body }, bytecode))
    }
}

#[cfg(test)]
mod tests {
    use crate::{MPCProtocol, MPCProtocolFactory};
    use anyhow::Error;
    use jit_compiler::{
        bytecode2protocol::Bytecode2Protocol,
        mir2bytecode::MIR2Bytecode,
        models::{bytecode::ProgramBytecode, protocols::ProtocolsModel},
    };
    use test_programs::PROGRAMS;

    pub(crate) fn compile_bytecode(program_id: &str) -> Result<ProgramBytecode, Error> {
        let mir = PROGRAMS.mir(program_id)?;
        let bytecode = MIR2Bytecode::transform(&mir)?;
        Ok(bytecode)
    }

    pub(crate) fn compile_protocols(program_id: &str) -> Result<ProtocolsModel<MPCProtocol>, Error> {
        let mir = PROGRAMS.mir(program_id)?;
        let bytecode = MIR2Bytecode::transform(&mir)?;
        let program = Bytecode2Protocol::transform(MPCProtocolFactory, &bytecode)?;
        Ok(program)
    }
}
