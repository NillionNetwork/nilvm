//! This create implements the MIR to bytecode transformation.
mod mir2bytecode;

pub use mir2bytecode::*;

pub mod errors;
mod operations;

#[cfg(test)]
mod tests;
