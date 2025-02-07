//! This crate implements the support for the memory

use crate::models::memory::{AddressCountError, AddressType};
#[cfg(any(test, feature = "serde"))]
use serde::{Deserialize, Serialize};
use std::fmt::{Debug, Display, Formatter};

/// Implementation of a memory address
#[derive(Clone, Debug, Copy, Default, PartialEq, PartialOrd, Ord, Eq, Hash)]
#[cfg_attr(any(test, feature = "serde"), derive(Serialize, Deserialize))]
pub struct BytecodeAddress(pub usize, pub AddressType);

impl BytecodeAddress {
    /// Construct a new [`BytecodeAddress`]
    pub fn new(address: usize, memory_type: AddressType) -> Self {
        Self(address, memory_type)
    }

    /// Returns the next [`BytecodeAddress`]
    pub fn next(&self) -> Result<BytecodeAddress, BytecodeMemoryError> {
        self.advance(1)
    }

    /// Advance an offset number of addresses
    pub fn advance(&self, offset: usize) -> Result<BytecodeAddress, BytecodeMemoryError> {
        Ok(Self(self.0.checked_add(offset).ok_or(BytecodeMemoryError::Overflow)?, self.1))
    }

    /// Converts address into heap address
    pub fn as_heap(&self) -> BytecodeAddress {
        Self(self.0, AddressType::Heap)
    }
}

impl Display for BytecodeAddress {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}({})", self.1, self.0)
    }
}

/// Theses errors are thrown during the memory address calculation
#[derive(Debug, thiserror::Error)]
pub enum BytecodeMemoryError {
    /// Identifier counter is overflow
    #[error("identifier counter overflow")]
    IdentifierOverflow,

    /// Memory address is overflow
    #[error("memory address overflow")]
    Overflow,

    /// Memory address is underflow
    #[error("memory address underflow")]
    Underflow,

    /// Out of memory error
    #[error("out of memory {1:?}: {0}")]
    OutOfMemory(&'static str, BytecodeAddress),

    /// An address is used to access to the wrong type of memory
    #[error("illegal memory access")]
    IllegalMemoryAccess,

    /// This error is thrown where the address calculation for a type fails.
    #[error(transparent)]
    AddressCount(#[from] AddressCountError),
}

impl From<BytecodeAddress> for usize {
    fn from(value: BytecodeAddress) -> Self {
        value.0
    }
}
