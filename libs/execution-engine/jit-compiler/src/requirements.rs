//! Implements the abstractions for the requirements of a program.

use crate::{models::protocols::Protocol, Program};
use anyhow::Error;

/// Program requirements implementation
pub trait ProgramRequirements<P: Protocol>: Sized {
    /// Get all program requirements from the program
    fn from_program(program: &Program<P>) -> Result<Self, Error>;

    /// Update the runtime requirement type with a new count of elements
    fn with_runtime_requirements(self, element_type: P::RequirementType, count: usize) -> Self;

    /// Get the requirements of a specific type
    fn runtime_requirement(&self, element_type: &P::RequirementType) -> usize;
}
