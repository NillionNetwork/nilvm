//! This crate implements the support for the protocol model memory

use std::fmt::{Debug, Display, Formatter};

use crate::models::{bytecode::memory::BytecodeAddress, memory::AddressType};

/// Implementation of a memory address
#[derive(Clone, Debug, Copy, Default, PartialEq, PartialOrd, Ord, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ProtocolAddress(pub usize, pub AddressType);

impl ProtocolAddress {
    /// Construct a new [`ProtocolAddress`]
    pub fn new(address: usize, memory_type: AddressType) -> Self {
        Self(address, memory_type)
    }

    /// Returns the next [`ProtocolAddress`]
    pub fn next(&self) -> Result<ProtocolAddress, ProtocolMemoryError> {
        self.advance(1)
    }

    /// Advance an offset number of addresses
    pub fn advance(&self, offset: usize) -> Result<ProtocolAddress, ProtocolMemoryError> {
        Ok(Self(self.0.checked_add(offset).ok_or(ProtocolMemoryError::Overflow)?, self.1))
    }

    /// Converts address into heap address
    pub fn as_heap(&self) -> Self {
        Self(self.0, AddressType::Heap)
    }
}

impl Display for ProtocolAddress {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}({})", self.1, self.0)
    }
}

/// These errors are thrown during the memory address calculation
#[derive(Debug, thiserror::Error)]
pub enum ProtocolMemoryError {
    /// Memory address is overflow
    #[error("memory address overflow")]
    Overflow,
}

impl From<ProtocolAddress> for usize {
    fn from(value: ProtocolAddress) -> Self {
        value.0
    }
}

impl From<BytecodeAddress> for ProtocolAddress {
    fn from(value: BytecodeAddress) -> Self {
        Self(value.0, value.1)
    }
}

impl From<ProtocolAddress> for BytecodeAddress {
    fn from(value: ProtocolAddress) -> Self {
        Self(value.0, value.1)
    }
}
