//! The models for the Protocols representation of a program.

#[cfg(feature = "text_repr")]
use crate::models::text_repr_utils::{format_text_repr, snippet_with_loc};
use crate::models::{
    bytecode::Literal, memory::result_element_address_count, protocols::memory::ProtocolAddress, SourceFiles,
    SourceRef, SourceRefIndex,
};
use nada_type::NadaType;
#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, HashMap},
    fmt::{Debug, Display},
};

pub mod memory;
#[cfg(feature = "text_repr")]
pub mod text_repr;
mod utils;

/// Binary file extension for protocols body model
pub const PROTOCOLS_BODY_FILE_EXTENSION_BIN: &str = ".body.bin";
/// Json file extension for protocols body model
pub const PROTOCOLS_BODY_FILE_EXTENSION_JSON: &str = ".body.json";

/// Contains the information about the memory allocation where an input will be stored.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct InputMemoryAllocation {
    /// Input
    pub input: String,
    /// Reserved memory addresses
    pub sizeof: usize,
}

/// Contains the information about the memory allocation where is stored an output.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct OutputMemoryAllocation {
    /// Memory address where is allocated the output in the runtime memory.
    pub address: ProtocolAddress,
    /// Type of the output
    pub ty: NadaType,
}

/// Contains the information about the inputs distribution in the runtime memory.
pub type InputMemoryScheme = BTreeMap<ProtocolAddress, InputMemoryAllocation>;

/// Contains the information about the outputs distribution in the runtime memory.
pub type OutputMemoryScheme = BTreeMap<String, OutputMemoryAllocation>;

/// The program body
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct ProtocolsModel<P: Protocol> {
    /// Contains the information about how the inputs will be stored in the memory during
    /// the program execution
    pub input_memory_scheme: InputMemoryScheme,
    /// Contains the information about how the outputs are allocated in the memory. The main
    /// purpose is to resolve the names of them because of the outputs' name are not stored in the runtime
    /// memory.
    pub output_memory_scheme: OutputMemoryScheme,
    /// Literals.
    pub literals: Vec<Literal>,
    /// The body is represented by a tree of protocol/circuits
    pub protocols: BTreeMap<ProtocolAddress, P>,
    /// Source code info about the program.
    pub source_files: SourceFiles,
    /// Array of source references
    pub source_refs: Vec<SourceRef>,
    /// Counts the references to any address
    pub reads_table: HashMap<ProtocolAddress, usize>,
}

impl<P: Protocol> Default for ProtocolsModel<P> {
    fn default() -> Self {
        Self {
            input_memory_scheme: Default::default(),
            output_memory_scheme: Default::default(),
            literals: vec![],
            protocols: BTreeMap::new(),
            source_files: Default::default(),
            source_refs: vec![],
            reads_table: Default::default(),
        }
    }
}

impl<P: Protocol> ProtocolsModel<P> {
    /// Returns a new ProtocolsModel with associated source files and refs.
    pub fn new(source_files: SourceFiles, source_refs: Vec<SourceRef>) -> Self {
        Self {
            input_memory_scheme: InputMemoryScheme::default(),
            output_memory_scheme: OutputMemoryScheme::default(),
            literals: Vec::new(),
            protocols: BTreeMap::new(),
            source_files,
            source_refs,
            reads_table: HashMap::new(),
        }
    }

    /// Returns the required memory size
    pub fn memory_size(&self) -> usize {
        self.protocols
            .iter()
            .next_back()
            .map(|(address, protocol)| {
                let ty = protocol.ty();
                let offset = address.0;
                offset.wrapping_add(result_element_address_count(ty))
            })
            .unwrap_or_default()
    }
}

/// Execution line defines if a protocol is executed local or online
#[derive(Ord, PartialOrd, Eq, PartialEq, Copy, Clone, Default)]
pub enum ExecutionLine {
    /// The protocol does not require communication for its execution.
    #[default]
    Local = 0,
    /// The protocol requires communication for its execution.
    Online = 1,
}

/// Protocol implementation
pub trait Protocol: ProtocolDependencies + Debug + Clone + Display {
    /// Protocol requirement type
    type RequirementType: Debug;

    /// Returns the output type of the protocol
    fn ty(&self) -> &NadaType;

    /// Return the name of the protocol
    fn name(&self) -> &'static str;

    /// Assigns an address to the protocol
    fn with_address(&mut self, address: ProtocolAddress);

    /// Returns the address of the protocol
    fn address(&self) -> ProtocolAddress;

    /// Returns the runtime elements requirements

    fn runtime_requirements(&self) -> &[(Self::RequirementType, usize)];

    /// Returns the execution line for a protocol
    fn execution_line(&self) -> ExecutionLine;

    /// Return the SourceRefIndex of the protocol model element
    fn source_ref_index(&self) -> &SourceRefIndex;

    /// Teturns the text representation of the protocol
    #[cfg(feature = "text_repr")]
    fn text_repr(&self, program: &ProtocolsModel<Self>) -> String {
        let source_ref_index = self.source_ref_index();
        let snippet = snippet_with_loc(&program.source_files, &program.source_refs, source_ref_index)
            .unwrap_or_default()
            .unwrap_or_default();
        let incoming_references = program.reads_table.get(&self.address()).copied().unwrap_or_default();
        format_text_repr(self.to_string(), incoming_references, snippet)
    }
}

#[macro_export]
/// Builds a typed protocol for a protocol
macro_rules! protocol {
    ($name:ident, $requirement_type:ty, $requirements:expr, $execution_line:expr) => {
        impl $crate::models::protocols::Protocol for $name {
            type RequirementType = $requirement_type;

            fn ty(&self) -> &nada_value::NadaType {
                &self.ty
            }

            fn name(&self) -> &'static str {
                stringify!($name)
            }

            fn with_address(&mut self, address: $crate::models::protocols::memory::ProtocolAddress) {
                self.address = address;
            }

            fn address(&self) -> $crate::models::protocols::memory::ProtocolAddress {
                self.address
            }

            fn runtime_requirements(&self) -> &[(Self::RequirementType, usize)] {
                $requirements
            }

            fn execution_line(&self) -> $crate::models::protocols::ExecutionLine {
                $execution_line
            }

            fn source_ref_index(&self) -> &$crate::models::SourceRefIndex {
                &self.source_ref_index
            }
        }
    };
}

/// Protocol dependencies
pub trait ProtocolDependencies {
    /// Returns a vector that contains the addresses of the protocol's dependencies
    fn dependencies(&self) -> Vec<ProtocolAddress>;
}
