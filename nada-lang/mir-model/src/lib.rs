//! This crate implements the MIR model

#[cfg(feature = "builder")]
pub mod builder;

mod model;
mod utils;

#[cfg(feature = "proto")]
pub mod proto;

pub use model::*;
use nada_type::NadaType;
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, fmt::Display};
pub use utils::MIRProgramMalformed;

/// Binary file extension for MIR model
pub const MIR_FILE_EXTENSION_BIN: &str = ".nada.bin";
/// Json file extension for MIR model
pub const MIR_FILE_EXTENSION_JSON: &str = ".nada.json";

/// Operation ID
#[derive(Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash, Clone, Copy, Debug)]
pub struct OperationId(i64);

impl Display for OperationId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match *self {
            Self::INVALID => write!(f, "invalid operation ID"),
            id => write!(f, "{id:?}"),
        }
    }
}

impl OperationId {
    const FIRST: OperationId = OperationId(0);
    const INVALID: OperationId = OperationId(-1);

    /// Returns an operation ID with a provided ID.
    /// This should only be used during testing.
    pub fn with_id(id: i64) -> Self {
        Self(id)
    }

    /// Returns true if an operation ID is valid.
    /// Invalid operation IDs are returned by calling the default
    pub fn is_valid(&self) -> bool {
        *self != Self::INVALID
    }
}

/// Operation ID generator. Generates operation ID incrementally.
#[derive(Debug, Clone, Default)]
pub struct OperationIdGenerator {
    current_id: i64,
}

impl OperationIdGenerator {
    /// Creates a new operation ID generator with a provided first ID.
    pub fn with_next(id: OperationId) -> Self {
        Self { current_id: id.0 }
    }

    /// Returns a new operation ID.
    pub fn next_id(&mut self) -> OperationId {
        let current_id = self.current_id;
        self.current_id = self.current_id.wrapping_add(1);
        OperationId(current_id)
    }
}

/// Represents a model element with source info
pub trait SourceInfo {
    /// Source reference information of this element
    fn source_ref_index(&self) -> SourceRefIndex;
}

#[macro_export]
/// Implements SourceInfo trait
macro_rules! source_info {
    ($name:ident) => {
        impl SourceInfo for $name {
            fn source_ref_index(&self) -> SourceRefIndex {
                self.source_ref_index
            }
        }
    };
}

/// Represents an element with name
pub trait NamedElement {
    /// Returns the name of the element
    fn name(&self) -> &str;
}

#[macro_export]
/// Build a named element
macro_rules! named_element {
    ($name:ident) => {
        impl NamedElement for $name {
            fn name(&self) -> &str {
                &self.name
            }
        }
    };
    ($name:ident, $name_str:literal) =>{

        impl NamedElement for $name {
            fn name(&self) -> &str {
                $name_str
            }
        }

    };

    ($(($name:ident, $name_str:literal)),+) =>{
        $(
        impl NamedElement for $name {
            fn name(&self) -> &str {
                $name_str
            }
        }
    )+
    };
}

/// Represents the output of a program
pub trait OutputElement: NamedElement + TypedElement {
    /// Returns the party of an output
    fn party(&self) -> &str;
}

/// Build an output
#[macro_export]
macro_rules! output_element {
    ($name:ident) => {
        impl OutputElement for $name {
            fn party(&self) -> &str {
                &self.party
            }
        }
    };
}

/// Represents a typed element
pub trait TypedElement {
    /// Returns the element's type
    fn ty(&self) -> &NadaType;
}

/// Build a typed element
#[macro_export]
macro_rules! typed_element {
    ($name:ident) => {
        impl TypedElement for $name {
            fn ty(&self) -> &NadaType {
                &self.ty
            }
        }
    };
}

source_info!(Party);
named_element!(Party);

/// Represents an element with an identifier
pub trait IdentifiableElement {
    /// Type of the identifier
    type Id;
    /// Returns the identifier of the element
    fn id(&self) -> Self::Id;
    /// Sets the identifier for the element
    fn set_id(&mut self, id: Self::Id);
}

/// Represents an element that has an identifier
#[macro_export]
macro_rules! identifiable_element {
    ($name:ident, $ty:ident) => {
        impl IdentifiableElement for $name {
            type Id = $ty;

            fn id(&self) -> Self::Id {
                self.id
            }

            fn set_id(&mut self, id: Self::Id) {
                self.id = id
            }
        }
    };
}

/// Represents a binary operation
pub trait UnaryOperation: IdentifiableElement + TypedElement + NamedElement + SourceInfo {
    /// Returns the operand of the operation
    fn operand(&self) -> OperationId;
}

/// Represents a binary operation
pub trait BinaryOperation: IdentifiableElement + TypedElement + NamedElement + SourceInfo {
    /// Returns the left operand of the operation
    fn left(&self) -> OperationId;
    /// Returns the right operand of the operation
    fn right(&self) -> OperationId;
    /// Returns true if this operation has a public-only output
    fn public_output_only(&self) -> bool;
}

/// Implemented by operations that have operands.
pub trait HasOperands {
    /// Returns a Vec of operands for this operation.
    fn operands(&self) -> Vec<OperationId>;
}

/// Party type that is used to identify who is the owner of an input.
#[derive(Deserialize, Debug, Clone, Serialize, Eq, Hash, PartialEq)]
pub struct Party {
    /// Name of the party
    pub name: String,
    /// Source reference
    pub source_ref_index: SourceRefIndex,
}

/// MIR Input
#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
#[cfg_attr(any(test, feature = "builder"), derive(Hash))]
pub struct Input {
    /// Operation type
    #[serde(rename = "type")]
    pub ty: NadaType,
    /// Party contains this input
    pub party: String,
    /// Input name
    pub name: String,
    /// The documentation.
    pub doc: String,
    /// Source file info related with this operation.
    pub source_ref_index: SourceRefIndex,
}
named_element!(Input);
source_info!(Input);
typed_element!(Input);

/// MIR NADA Literal
#[derive(Serialize, Deserialize, Debug, Hash, Eq, PartialEq, Clone)]
pub struct Literal {
    /// Name
    pub name: String,
    /// Value
    pub value: String,
    /// Type
    #[serde(rename = "type")]
    pub ty: NadaType,
}

/// Information about Nada-lang file that contains an element
#[derive(Deserialize, Debug, Clone, Serialize, Eq, Hash, PartialEq, Default)]
pub struct SourceRef {
    /// Nada-lang file that contains the elements
    pub file: String,
    /// Line number into the file that contains the element
    pub lineno: u32,
    /// Element's offset into the file
    pub offset: u32,
    /// Element's length into the file
    pub length: u32,
}

/// Sources Files contains all used files and the content of them
pub type SourceFiles = BTreeMap<String, String>;

/// Index to a source ref
#[derive(Deserialize, Debug, Clone, Copy, Serialize, Eq, Hash, PartialEq, Default)]
pub struct SourceRefIndex(pub u64);
