//! Client core library.
//!
//! This is a thin core that contains the shared code among all of the supported clients.

#![deny(exported_private_dependencies, missing_docs)]

pub mod values;
pub use ecdsa_keypair::{generic_ec, privatekey, signature};
pub use key_share;

/// Programs utilities
pub mod programs {
    pub use mpc_vm::requirements::{MPCProgramRequirements, ProgramRequirements, RuntimeRequirementType};
    pub use program_auditor::{ProgramAuditorError, ProgramAuditorRequest};

    /// Extract the program metadata to be used when uploading a program.
    pub fn extract_program_metadata(program: &[u8]) -> Result<ProgramAuditorRequest, ProgramAuditorError> {
        ProgramAuditorRequest::from_raw_mir(program)
    }
}
