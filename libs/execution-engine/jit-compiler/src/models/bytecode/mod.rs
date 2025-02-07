//! This crate implements the bytecode model

#[cfg(feature = "text_repr")]
use anyhow::{anyhow, Error};

#[cfg(any(test, feature = "serde"))]
use serde::{Deserialize, Serialize};

use crate::{
    models::{
        bytecode::{
            memory::{BytecodeAddress, BytecodeMemoryError},
            utils::{binary_operation_bytecode, ternary_operation_bytecode, unary_operation_bytecode},
        },
        memory::{address_count, AddressType, AddressType::Heap},
        Party, SourceFiles, SourceRef, SourceRefIndex,
    },
    source_info,
};
use enum_dispatch::enum_dispatch;
use itertools::Itertools;
pub use nada_compiler_backend::literal_value::LiteralValue;
use nada_compiler_backend::mir::{named_element, typed_element, NamedElement, TypedElement};
use nada_type::NadaType;
use std::{
    collections::BTreeMap,
    fmt::{Debug, Display, Formatter},
};

#[cfg(any(test, feature = "builder"))]
pub mod builder;
pub mod memory;
#[macro_use]
pub(crate) mod utils;
#[cfg(feature = "text_repr")]
pub mod text_repr;

/// Binary file extension for bytecode model
pub const BYTECODE_FILE_EXTENSION_BIN: &str = ".nada-bytecode-circuit.bin";
/// Json file extension for bytecode model
pub const BYTECODE_FILE_EXTENSION_JSON: &str = ".nada-bytecode-circuit.json";

#[enum_dispatch]
pub(crate) trait AddressedElement: TypedElement {
    fn address(&self) -> BytecodeAddress;

    fn with_address(self, address: BytecodeAddress) -> Self;
}

#[macro_export]
/// This macro is used for the operations for implementing the trait named ['AddressedElement']
macro_rules! addressed_operation {
    ($element:ident) => {
        impl AddressedElement for $element {
            fn address(&self) -> BytecodeAddress {
                self.address
            }

            fn with_address(mut self, address: BytecodeAddress) -> Self {
                self.address = address;
                self
            }
        }
    };
}

/// Represents the Memory where the variables are allocated. Currently, we have 2 types of memory:
/// - input contains the program's inputs
/// - output contains the program's outputs
#[derive(Clone, Debug)]
#[cfg_attr(any(test, feature = "serde"), derive(Serialize, Deserialize))]
#[cfg_attr(test, derive(PartialEq))]
pub struct BytecodeMemory {
    inputs: MemoryPool<Input>,
    outputs: MemoryPool<Output>,
    /// This vector contains all program's literals
    literals: Vec<Literal>,
    heap: Vec<Operation>,
}

impl Default for BytecodeMemory {
    fn default() -> Self {
        BytecodeMemory {
            inputs: MemoryPool::new(AddressType::Input),
            outputs: MemoryPool::new(AddressType::Output),
            literals: Vec::new(),
            heap: Vec::new(),
        }
    }
}

impl BytecodeMemory {
    /// Get a list of all literals in the memory
    pub fn literals(&self) -> impl Iterator<Item = &Literal> {
        self.literals.iter()
    }

    /// Add a new literal into the literal memory
    pub fn add_literal(&mut self, literal: Literal) {
        self.literals.push(literal)
    }

    /// Get the literal allocated in the provided address
    pub fn literal(&self, address: BytecodeAddress) -> Result<Option<&Literal>, BytecodeMemoryError> {
        match address.1 {
            AddressType::Literals => Ok(self.literals.get::<usize>(address.into())),
            _ => Err(BytecodeMemoryError::IllegalMemoryAccess),
        }
    }

    /// Add a new input into the input memory
    pub fn add_input(&mut self, input: Input) -> Result<BytecodeAddress, BytecodeMemoryError> {
        self.inputs.push(input)
    }

    /// Get a list of all inputs in the memory
    pub fn inputs(&self) -> impl Iterator<Item = &Input> {
        self.inputs.elements()
    }

    /// Get the input allocated in the provided address
    pub fn input(&self, address: BytecodeAddress) -> Result<Option<&Input>, BytecodeMemoryError> {
        self.inputs.element(address)
    }

    /// Returns the memory address of an input, if it exists. If the memory contains several occurrences
    /// of it, it returns the first one
    pub(crate) fn inputs_address(&self, input_name: &str) -> Option<BytecodeAddress> {
        self.inputs.memory_address(input_name)
    }

    /// Returns the number of inputs
    pub fn inputs_count(&self) -> usize {
        self.inputs.elements_count()
    }

    /// Returns the size of the input memory
    pub fn input_memory_size(&self) -> usize {
        self.inputs.memory_size()
    }

    /// Add a new output into the output memory
    pub fn add_output(&mut self, output: Output) -> Result<BytecodeAddress, BytecodeMemoryError> {
        self.outputs.push(output)
    }

    /// Get a list of all outputs in the memory
    pub fn outputs(&self) -> impl Iterator<Item = &Output> {
        self.outputs.elements()
    }

    /// Returns the number of outputs
    pub fn outputs_count(&self) -> usize {
        self.outputs.elements_count()
    }

    /// Returns the size of the output memory
    pub fn output_memory_size(&self) -> usize {
        self.outputs.memory_size()
    }

    /// Add a new operation into the operation memory
    pub fn add_operation(&mut self, operation: Operation) -> BytecodeAddress {
        let address = BytecodeAddress::new(self.operations_count(), Heap);
        self.heap.push(operation);
        address
    }

    /// Get the operation allocated in the provided address
    pub fn operation(&self, address: BytecodeAddress) -> Result<Option<&Operation>, BytecodeMemoryError> {
        match address.1 {
            Heap => Ok(self.heap.get::<usize>(address.into())),
            _ => Err(BytecodeMemoryError::IllegalMemoryAccess),
        }
    }

    /// Get a list of all operations in the memory
    pub fn operations(&self) -> impl Iterator<Item = &Operation> {
        self.heap.iter()
    }

    /// Returns the number of operations
    pub fn operations_count(&self) -> usize {
        self.heap.len()
    }

    /// Returns the type of an element is allocated into any bytecode memory
    pub fn memory_element_type(&self, address: BytecodeAddress) -> Result<&NadaType, BytecodeMemoryError> {
        use AddressType::*;
        match address.1 {
            Input => Ok(self.inputs.ty(address)?),
            Output => Ok(self.outputs.ty(address)?),
            Heap => match self.operation(address)? {
                Some(op) => Ok(op.ty()),
                None => {
                    Err(BytecodeMemoryError::OutOfMemory("operation not found, trying to get element type", address))
                }
            },

            Literals => {
                Ok(self.literal(address)?.ok_or(BytecodeMemoryError::OutOfMemory("literal not found", address))?.ty())
            }
        }
    }

    /// Return the addresses of the compound type with the given address.
    pub fn inner_addresses(
        &self,
        address: BytecodeAddress,
    ) -> Result<impl Iterator<Item = BytecodeAddress>, BytecodeMemoryError> {
        use AddressType::*;
        let inner_addresses = match address.1 {
            Input => self.inputs.get_children_addresses(address)?,
            Output => self.outputs.get_children_addresses(address)?,
            Heap => {
                let element =
                    self.operation(address)?.ok_or(BytecodeMemoryError::OutOfMemory("operation not found", address))?;
                get_children_addresses(element, address)?
            }
            _ => Err(BytecodeMemoryError::IllegalMemoryAccess)?,
        };
        Ok(inner_addresses.into_iter())
    }
}

/// Represent a pool of memory that contains all used piece of data by the program.
#[derive(Clone, Debug)]
#[cfg_attr(any(test, feature = "serde"), derive(Serialize, Deserialize))]
#[cfg_attr(test, derive(PartialEq))]
pub(crate) struct MemoryPool<E: AddressedElement + NamedElement> {
    /// Indicates the type of the memory
    memory_type: AddressType,
    /// Contains the allocated elements
    memory: Vec<MemoryElement>,
    /// Index to the elements that are allocated in the memory
    element_index: BTreeMap<usize, E>,
}

impl<E: AddressedElement + NamedElement> MemoryPool<E> {
    pub(crate) fn new(memory_type: AddressType) -> Self {
        MemoryPool { memory_type, element_index: BTreeMap::new(), memory: Vec::new() }
    }

    pub(crate) fn push(&mut self, element: E) -> Result<BytecodeAddress, BytecodeMemoryError> {
        let address: BytecodeAddress = BytecodeAddress::new(self.memory.len(), self.memory_type);
        let element = element.with_address(address);
        self.allocate_element(element.ty().clone())?;
        self.element_index.insert(address.into(), element);
        Ok(address)
    }

    fn allocate_element(&mut self, element_ty: NadaType) -> Result<(), BytecodeMemoryError> {
        // This vector contains a tuple with the container element and the element we have to allocate.
        // The first element is the root and is not contain for anyother element
        let mut remaining_element_types = vec![(None, element_ty)];
        while let Some((container, ty)) = remaining_element_types.pop() {
            let container_address = BytecodeAddress::new(self.memory.len(), self.memory_type);
            let inner_elements: Vec<_> =
                self.inner_element_types(&ty).into_iter().map(|inner| (Some(container_address), inner)).collect();
            remaining_element_types.extend(inner_elements);
            self.memory.push(MemoryElement { container, ty });
        }
        Ok(())
    }

    /// Returns the types of the inner elements
    fn inner_element_types(&self, ty: &NadaType) -> Vec<NadaType> {
        use NadaType::*;
        match ty {
            // An array contains as many inner elements as its size.
            Array { size, inner_type, .. } => {
                vec![inner_type.as_ref().clone(); *size]
            }

            // A tuple contains two inner elements.
            Tuple { left_type, right_type } => {
                vec![right_type.as_ref().clone(), left_type.as_ref().clone()]
            }

            // An n tuple contains as many inner elements as its types.
            NTuple { types } => types.clone(),

            // An n tuple contains as many inner elements as its types.
            Object { types } => types.values().cloned().collect_vec(),

            Integer
            | UnsignedInteger
            | Boolean
            | EcdsaDigestMessage
            | SecretInteger
            | SecretUnsignedInteger
            | SecretBoolean
            | SecretBlob
            | ShamirShareInteger
            | ShamirShareUnsignedInteger
            | ShamirShareBoolean
            | EcdsaPrivateKey
            | EcdsaSignature => vec![],
        }
    }

    pub(crate) fn elements_count(&self) -> usize {
        self.element_index.len()
    }

    pub(crate) fn memory_size(&self) -> usize {
        self.memory.len()
    }

    pub(crate) fn elements(&self) -> impl Iterator<Item = &E> {
        self.element_index.values()
    }

    /// Get the element allocated in the provided address
    pub fn element(&self, address: BytecodeAddress) -> Result<Option<&E>, BytecodeMemoryError> {
        if self.memory_type == address.1 {
            let mut element_address = address;
            let mut memory_element = self
                .memory
                .get::<usize>(address.into())
                .ok_or(BytecodeMemoryError::OutOfMemory("element not found", address))?;
            while let Some(container) = memory_element.container {
                element_address = container;
                memory_element = self
                    .memory
                    .get::<usize>(element_address.into())
                    .ok_or(BytecodeMemoryError::OutOfMemory("element not found", address))?;
            }
            Ok(self.element_index.get(&element_address.into()))
        } else {
            Err(BytecodeMemoryError::IllegalMemoryAccess)
        }
    }

    /// Return the first address where the element is allocated, if it exists in the memory.
    /// If the element is allocated several times, this method returns only the first occurrence
    pub fn memory_address(&self, element_name: &str) -> Option<BytecodeAddress> {
        self.element_index
            .iter()
            .find(|(_, e)| e.name() == element_name)
            .map(|(i, _)| BytecodeAddress(*i, self.memory_type))
    }

    /// The type of the element stored at the given address
    pub fn ty(&self, address: BytecodeAddress) -> Result<&NadaType, BytecodeMemoryError> {
        self.memory
            .get::<usize>(address.into())
            .map(|element| &element.ty)
            .ok_or(BytecodeMemoryError::OutOfMemory("element not found", address))
    }

    /// Get Children addresses for a compound element.
    pub fn get_children_addresses(
        &self,
        address: BytecodeAddress,
    ) -> Result<Vec<BytecodeAddress>, BytecodeMemoryError> {
        let element = self.element(address)?.ok_or(BytecodeMemoryError::OutOfMemory("element not found", address))?;
        get_children_addresses(element, address)
    }
}

fn collect_addresses<'a, I>(address: BytecodeAddress, types: I) -> Result<Vec<BytecodeAddress>, BytecodeMemoryError>
where
    I: IntoIterator<Item = &'a NadaType>,
{
    let mut addresses = Vec::new();
    let mut next_address = address.next()?;
    for ty in types {
        addresses.push(next_address);
        let count = address_count(ty)?;
        next_address = next_address.advance(count)?;
    }
    Ok(addresses)
}

/// Get Children Element Addresses.
///
/// Provided a [`BytecodeAddress`] that is allocated to a NADA compound type ([`Array`], [`Vector`], [`Tuple`]...),
/// this method will recover the addresses of all the direct children elements of the compound type. That is,
/// if the type of the children of the compound type is another compound type, the 'children of the children' will not be inspected.
/// Only the direct children of the compount type are analyzed.
///
/// The returned value is a vector with all the memory addresses that are contained in the provided type. For an Array of size
/// N, the returned value is a vector of size N with all the memory addresses of its N elements.
///
/// For Arrays, given that the inner type and the size is known at compilation time, the following logic is applied:
/// - First, the size of the children (or inner) elements is calculated using [`address_count`].
/// - Next, the first element is located at the array address + 1
/// - Then loop over all the elements of the array, advancing for each element a number of steps equal to its size. In other words,
///   if the size for an element is S, the address of the second element is = address of the first elememt + S. The address of the third
///   element will be the address of the second element + S. And so on.
///
/// This is what an Array looks like in Memory: | Array Address | First Element Address | ..(Size of Array Element).. | Second Element Address| ...
///
/// Similarly, in the case of Tuples, the left element is addressed at the Tuple address + 1. The right element is addressed
/// at the address of the left element plus it's size.
///
/// A Tuple looks like this in Memory: | Tuple Address | Left Element Address | ... (Size of Left Element) | Right Element Address| ...
fn get_children_addresses<T: TypedElement>(
    element: &T,
    address: BytecodeAddress,
) -> Result<Vec<BytecodeAddress>, BytecodeMemoryError> {
    use NadaType::*;
    match element.ty() {
        Array { size, inner_type } => {
            // Create an iterator that repeats the inner type `size` times
            let types = std::iter::repeat(inner_type.as_ref()).take(*size);
            let addresses = collect_addresses(address, types)?;
            Ok(addresses)
        }

        Tuple { left_type, right_type } => {
            // Create a vector of the tuple's element types
            let types = &[&**left_type, &**right_type];
            let addresses = collect_addresses(address, types.iter().copied())?;
            Ok(addresses)
        }

        NTuple { types } => {
            let addresses = collect_addresses(address, types.iter())?;
            Ok(addresses)
        }

        Object { types } => {
            let mut inner_addresses = vec![];
            // The address for the first position into the array is the array address + 1
            let mut next_inner_address = address.next()?;
            for inner_type in types.values() {
                inner_addresses.push(next_inner_address);
                // This is the size of each element of the array
                let inner_type_sizeof = address_count(inner_type)?;
                // The next element address is the current plus the size of an element.
                next_inner_address = next_inner_address.advance(inner_type_sizeof)?;
            }
            Ok(inner_addresses)
        }

        Integer
        | UnsignedInteger
        | Boolean
        | EcdsaDigestMessage
        | SecretInteger
        | SecretUnsignedInteger
        | SecretBoolean
        | SecretBlob
        | ShamirShareInteger
        | ShamirShareUnsignedInteger
        | ShamirShareBoolean
        | EcdsaPrivateKey
        | EcdsaSignature => Ok(vec![]),
    }
}

#[derive(Clone, Debug)]
#[cfg_attr(any(test, feature = "serde"), derive(Serialize, Deserialize))]
#[cfg_attr(test, derive(PartialEq))]
pub(crate) struct MemoryElement {
    ty: NadaType,
    container: Option<BytecodeAddress>,
}

/// Bytecode model output
#[derive(Clone, Debug)]
#[cfg_attr(any(test, feature = "serde"), derive(Serialize, Deserialize))]
#[cfg_attr(test, derive(PartialEq))]
pub struct Output {
    /// It's own address in output memory
    pub address: BytecodeAddress,
    /// Output name
    pub name: String,
    /// Address of the inner operation in the program's operations vector.
    pub inner: BytecodeAddress,
    /// Party Id that contains this output. This id matches the address of the party in the program's parties vector.
    pub party_id: usize,
    /// Output type
    #[cfg_attr(any(test, feature = "serde"), serde(rename = "type"))]
    pub ty: NadaType,
    /// Source code info about this element.
    pub source_ref_index: SourceRefIndex,
}
source_info!(Output);
named_element!(Output);
typed_element!(Output);

impl AddressedElement for Output {
    fn address(&self) -> BytecodeAddress {
        self.address
    }

    fn with_address(mut self, address: BytecodeAddress) -> Self {
        self.address = address;
        self
    }
}

impl Output {
    /// Creates a new Output
    pub fn new(
        party_id: usize,
        name: String,
        inner: BytecodeAddress,
        ty: NadaType,
        source_ref_index: SourceRefIndex,
    ) -> Self {
        Output { address: BytecodeAddress(0usize, AddressType::Output), party_id, name, inner, ty, source_ref_index }
    }
}

impl Display for Output {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} rty({}) = Output({}) {}", self.address, self.ty, self.name, self.inner)
    }
}

/// Bytecode model input
#[derive(Clone, Debug)]
#[cfg_attr(any(test, feature = "serde"), derive(Serialize, Deserialize))]
#[cfg_attr(test, derive(PartialEq))]
pub struct Input {
    /// Party Id that contains this input. This id matches the address of the party in the program's parties vector.
    pub party_id: usize,
    /// Input name
    pub name: String,
    /// Its own address in input memory
    pub address: BytecodeAddress,
    /// Input type
    #[cfg_attr(any(test, feature = "serde"), serde(rename = "type"))]
    pub ty: NadaType,
    /// Source code info about this element.
    pub source_ref_index: SourceRefIndex,
}
source_info!(Input);
named_element!(Input);
typed_element!(Input);

impl AddressedElement for Input {
    fn address(&self) -> BytecodeAddress {
        self.address
    }

    fn with_address(mut self, address: BytecodeAddress) -> Self {
        self.address = address;
        self
    }
}

impl Input {
    /// Creates a new Input
    pub fn new(party_id: usize, name: String, ty: NadaType, source_ref_index: SourceRefIndex) -> Self {
        Input { address: BytecodeAddress(0usize, AddressType::Input), party_id, name, ty, source_ref_index }
    }
}

impl Display for Input {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} rty({}) = Input({})", self.address, self.ty, self.name)
    }
}

/// Bytecode model literal
#[derive(Clone, Debug)]
#[cfg_attr(any(test, feature = "serde"), derive(Serialize, Deserialize))]
#[cfg_attr(test, derive(PartialEq))]
pub struct LiteralRef {
    /// Literal id. This id matches the index of the literal in the program's literals vector
    pub literal_id: BytecodeAddress,
    /// Address of this literal in the program's operations vector.
    pub address: BytecodeAddress,
    /// Literal type
    #[cfg_attr(any(test, feature = "serde"), serde(rename = "type"))]
    pub ty: NadaType,
    /// Source code info about this element.
    pub source_ref_index: SourceRefIndex,
}

impl Display for LiteralRef {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} rty({}) = LiteralRef {}", self.address, self.ty, self.literal_id)
    }
}
source_info!(LiteralRef);
typed_element!(LiteralRef);
addressed_operation!(LiteralRef);

/// Bytecode model literal
#[derive(Clone, Debug)]
#[cfg_attr(any(test, feature = "serde"), derive(Serialize, Deserialize))]
#[cfg_attr(test, derive(PartialEq))]
pub struct Literal {
    /// Literal name
    pub name: String,
    /// Value.
    pub value: LiteralValue,
    /// Literal type
    #[cfg_attr(any(test, feature = "serde"), serde(rename = "type"))]
    pub ty: NadaType,
}

impl Literal {
    /// Retruns the literal's type
    pub fn ty(&self) -> &NadaType {
        &self.ty
    }
}

impl Display for Literal {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "rty({}) = Literal({}) val({:?})", self.ty, self.name, self.value)
    }
}

/// The bytecode for a Nada program.
#[derive(Clone, Debug, Default)]
#[cfg_attr(any(test, feature = "serde"), derive(Serialize, Deserialize))]
#[cfg_attr(test, derive(PartialEq))]
pub struct ProgramBytecode {
    /// This vector contains the parties that contains any input involved in the program's execution.
    pub parties: Vec<Party>,
    /// Program bytecode memory
    memory: BytecodeMemory,
    /// Source code info about the circuit.
    pub source_files: SourceFiles,
    /// Array of source references
    pub source_refs: Vec<SourceRef>,
}

impl ProgramBytecode {
    pub(crate) fn with_source_files(mut self, source_files: SourceFiles) -> Self {
        self.source_files = source_files;
        self
    }

    pub(crate) fn with_source_refs(mut self, source_refs: Vec<SourceRef>) -> Self {
        self.source_refs = source_refs;
        self
    }

    #[cfg(feature = "text_repr")]
    pub(crate) fn source_ref(&self, index: SourceRefIndex) -> Result<&SourceRef, Error> {
        self.source_refs.get(index.0 as usize).ok_or(anyhow!("source ref with index {} not found", index.0))
    }

    /// Returns all inputs
    pub fn literals(&self) -> impl Iterator<Item = &Literal> {
        self.memory.literals()
    }

    pub(crate) fn add_literal(&mut self, literal: Literal) {
        self.memory.add_literal(literal)
    }

    /// Returns the allocated literal in the specific address
    pub fn literal(&self, address: BytecodeAddress) -> Result<Option<&Literal>, BytecodeMemoryError> {
        self.memory.literal(address)
    }

    /// Returns the allocated input in the specific address
    pub fn input(&self, address: BytecodeAddress) -> Result<Option<&Input>, BytecodeMemoryError> {
        self.memory.input(address)
    }

    /// Returns the type of a memory element that is allocated in an specific address
    pub fn memory_element_type(&self, address: BytecodeAddress) -> Result<&NadaType, BytecodeMemoryError> {
        self.memory.memory_element_type(address)
    }

    /// Returns all inputs
    pub fn inputs(&self) -> impl Iterator<Item = &Input> {
        self.memory.inputs()
    }

    /// Returns the memory address of an input, if it exists. If the memory contains several occurrences
    /// of the input, this method returns only the first one
    pub fn input_address(&self, input_name: &str) -> Option<BytecodeAddress> {
        self.memory.inputs_address(input_name)
    }

    pub(crate) fn add_input(&mut self, input: Input) -> Result<BytecodeAddress, BytecodeMemoryError> {
        self.memory.add_input(input)
    }

    /// Returns the number of inputs
    pub fn inputs_count(&self) -> usize {
        self.memory.inputs_count()
    }

    /// Returns the size of the input memory
    pub fn input_memory_size(&self) -> usize {
        self.memory.input_memory_size()
    }

    /// Returns all outputs
    pub fn outputs(&self) -> impl Iterator<Item = &Output> {
        self.memory.outputs()
    }

    pub(crate) fn add_output(&mut self, output: Output) -> Result<BytecodeAddress, BytecodeMemoryError> {
        self.memory.add_output(output)
    }

    /// Returns the number of outputs
    pub fn outputs_count(&self) -> usize {
        self.memory.outputs_count()
    }

    /// Returns the size of the output memory
    pub fn output_memory_size(&self) -> usize {
        self.memory.output_memory_size()
    }

    pub(crate) fn add_operation(&mut self, operation: Operation) -> BytecodeAddress {
        self.memory.add_operation(operation)
    }

    /// Returns the operation with the specified address
    pub fn operation(&self, address: BytecodeAddress) -> Result<Option<&Operation>, BytecodeMemoryError> {
        self.memory.operation(address)
    }

    /// This function return an iterator over the program's operations.
    pub fn operations(&self) -> impl Iterator<Item = &Operation> {
        self.memory.operations()
    }

    /// Returns the number of operations
    pub fn operations_count(&self) -> usize {
        self.memory.operations_count()
    }

    /// Returns the inner elements addresses
    pub fn inner_addresses(
        &self,
        address: BytecodeAddress,
    ) -> Result<impl Iterator<Item = BytecodeAddress>, BytecodeMemoryError> {
        self.memory.inner_addresses(address)
    }
}

/// Bytecode operation types. New operations must be added in this enum as a new variant.
#[derive(Clone, Debug)]
#[cfg_attr(any(test, feature = "serde"), derive(Serialize, Deserialize))]
#[cfg_attr(test, derive(PartialEq))]
#[enum_dispatch(AddressedElement)]
#[repr(u8)]
pub enum Operation {
    /// Not operation variant
    Not(Not) = 0,
    /// Addition operation variant
    Addition(Addition) = 1,
    /// Subtraction operation variant
    Subtraction(Subtraction) = 2,
    /// Multiplication operation variant
    Multiplication(Multiplication) = 3,
    /// Cast operation variant
    Cast(Cast) = 4,
    /// Load an element from the input or literal memory to the heap memory.
    Load(Load) = 5,
    /// Load an element from a address of the heap.
    Get(Get) = 6,
    /// Create a new compound type.
    New(New) = 7,
    /// Modulo operation variant
    Modulo(Modulo) = 8,
    /// Power operation variant
    Power(Power) = 9,
    /// LeftShift operation variant
    LeftShift(LeftShift) = 10,
    /// RightShift operation variant
    RightShift(RightShift) = 11,
    /// Division operation variant
    Division(Division) = 12,
    /// Less than operation variant
    LessThan(LessThan) = 13,
    /// Equality protocol both for secret and public inputs
    Equals(Equals) = 14,
    /// Equals public output operation variant
    PublicOutputEquality(PublicOutputEquality) = 15,
    /// Literal references
    Literal(LiteralRef) = 16,
    /// IfElse operation variant
    IfElse(IfElse) = 18,
    /// Reveal operation variant
    Reveal(Reveal) = 19,
    /// Random operation variant
    Random(Random) = 20,
    /// Probabilistic truncation operation variant
    TruncPr(TruncPr) = 21,
    /// Inner product bytecode operation variant
    InnerProduct(InnerProduct) = 22,
    /// Inner product bytecode operation variant
    EcdsaSign(EcdsaSign) = 23,
}

impl Display for Operation {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Operation::Not(op) => write!(f, "{}", op),
            Operation::Addition(op) => write!(f, "{}", op),
            Operation::Subtraction(op) => write!(f, "{}", op),
            Operation::Multiplication(op) => write!(f, "{}", op),
            Operation::Cast(op) => write!(f, "{}", op),
            Operation::Load(op) => write!(f, "{}", op),
            Operation::Get(op) => write!(f, "{}", op),
            Operation::New(op) => write!(f, "{}", op),
            Operation::Modulo(op) => write!(f, "{}", op),
            Operation::Power(op) => write!(f, "{}", op),
            Operation::LeftShift(op) => write!(f, "{}", op),
            Operation::RightShift(op) => write!(f, "{}", op),
            Operation::Random(op) => write!(f, "{}", op),
            Operation::Division(op) => write!(f, "{}", op),
            Operation::LessThan(op) => write!(f, "{}", op),
            Operation::PublicOutputEquality(op) => write!(f, "{}", op),
            Operation::Equals(op) => write!(f, "{}", op),
            Operation::Literal(op) => write!(f, "{}", op),
            Operation::IfElse(op) => write!(f, "{}", op),
            Operation::Reveal(op) => write!(f, "{}", op),
            Operation::TruncPr(op) => write!(f, "{}", op),
            Operation::InnerProduct(op) => write!(f, "{}", op),
            Operation::EcdsaSign(op) => write!(f, "{}", op),
        }
    }
}

// TypedElement uses enum_dispatch, but it doesn't work if the trait is defined in a different crate.
// In this case, TypedElement is defined in the compiler-backend, for this reason, we have to implement
// the trait for bytecode::Operation. https://gitlab.com/antonok/enum_dispatch/-/issues/56
impl TypedElement for Operation {
    fn ty(&self) -> &NadaType {
        use Operation::*;
        match self {
            Not(op) => op.ty(),
            Addition(op) => op.ty(),
            Subtraction(op) => op.ty(),
            Multiplication(op) => op.ty(),
            Cast(op) => op.ty(),
            Load(op) => op.ty(),
            Get(op) => op.ty(),
            New(op) => op.ty(),
            Modulo(op) => op.ty(),
            Power(op) => op.ty(),
            LeftShift(op) => op.ty(),
            RightShift(op) => op.ty(),
            Random(op) => op.ty(),
            Division(op) => op.ty(),
            LessThan(op) => op.ty(),
            PublicOutputEquality(op) => op.ty(),
            Equals(op) => op.ty(),
            Literal(op) => op.ty(),
            IfElse(op) => op.ty(),
            Reveal(op) => op.ty(),
            TruncPr(op) => op.ty(),
            InnerProduct(op) => op.ty(),
            EcdsaSign(op) => op.ty(),
        }
    }
}

unary_operation_bytecode!(Not, "not");
unary_operation_bytecode!(Reveal, "reveal");

binary_operation_bytecode!(Addition, "addition");
binary_operation_bytecode!(Subtraction, "subtraction");
binary_operation_bytecode!(Multiplication, "multiplication");
binary_operation_bytecode!(Modulo, "modulo");
binary_operation_bytecode!(Power, "power");
binary_operation_bytecode!(LeftShift, "left-shift");
binary_operation_bytecode!(RightShift, "right-shift");
binary_operation_bytecode!(TruncPr, "trunc-pr");
binary_operation_bytecode!(Division, "division");
binary_operation_bytecode!(Equals, "equals");
binary_operation_bytecode!(LessThan, "less-than");
binary_operation_bytecode!(PublicOutputEquality, "public-output-equality");
binary_operation_bytecode!(InnerProduct, "inner-product");
binary_operation_bytecode!(EcdsaSign, "EcdsaSign");
ternary_operation_bytecode!(IfElse, "if-else");

/// Bytecode cast operation
#[derive(Clone, Debug)]
#[cfg_attr(any(test, feature = "serde"), derive(Serialize, Deserialize))]
#[cfg_attr(test, derive(PartialEq))]
pub struct Cast {
    /// Target type
    pub to: NadaType,
    /// Operation will be casted
    pub target: BytecodeAddress,
    /// Address of this operation in the program's operations vector.
    pub address: BytecodeAddress,
    /// Output type
    #[cfg_attr(any(test, feature = "serde"), serde(rename = "type"))]
    pub ty: NadaType,
    /// Source code info about this element.
    pub source_ref_index: SourceRefIndex,
}

impl Display for Cast {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} rty({}) = Cast {}, ty({})", self.address, self.to, self.target, self.ty)
    }
}
source_info!(Cast);
typed_element!(Cast);
addressed_operation!(Cast);

/// This element contains a reference to the input that is contains in program's inputs vector.
#[derive(Clone, Debug)]
#[cfg_attr(any(test, feature = "serde"), derive(Serialize, Deserialize))]
#[cfg_attr(test, derive(PartialEq))]
pub struct Random {
    /// Address of this operation in the program's operations vector.
    pub address: BytecodeAddress,
    /// Input type
    #[cfg_attr(any(test, feature = "serde"), serde(rename = "type"))]
    pub ty: NadaType,
    /// Source code info about this element.
    pub source_ref_index: SourceRefIndex,
}
impl Display for Random {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} rty({}) = Random", self.address, self.ty)
    }
}
source_info!(Random);
typed_element!(Random);
addressed_operation!(Random);

/// This element contains a reference to the input that is contains in program's inputs vector.
#[derive(Clone, Debug)]
#[cfg_attr(any(test, feature = "serde"), derive(Serialize, Deserialize))]
#[cfg_attr(test, derive(PartialEq))]
pub struct Load {
    /// Input id. This id matches the index of the input in the program's inputs vector
    pub input_address: BytecodeAddress,
    /// Address of this operation in the program's operations vector.
    pub address: BytecodeAddress,
    /// Input type
    #[cfg_attr(any(test, feature = "serde"), serde(rename = "type"))]
    pub ty: NadaType,
    /// Source code info about this element.
    pub source_ref_index: SourceRefIndex,
}
impl Display for Load {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} rty({}) = Load {}", self.address, self.ty, self.input_address)
    }
}
source_info!(Load);
typed_element!(Load);
addressed_operation!(Load);

/// This operation represents the loading of an element stored in the heap.
#[derive(Clone, Debug)]
#[cfg_attr(any(test, feature = "serde"), derive(Serialize, Deserialize))]
#[cfg_attr(test, derive(PartialEq))]
pub struct Get {
    /// Address of the heap from which the element is loaded
    pub source_address: BytecodeAddress,
    /// Address of this operation in the program's operations vector.
    pub address: BytecodeAddress,
    /// Input type
    #[cfg_attr(any(test, feature = "serde"), serde(rename = "type"))]
    pub ty: NadaType,
    /// Source code info about this element.
    pub source_ref_index: SourceRefIndex,
}

impl Display for Get {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} rty({}) = Get {}", self.address, self.ty, self.source_address)
    }
}
source_info!(Get);
typed_element!(Get);
addressed_operation!(Get);

/// Creates a new value of a composite type in the heap memory
#[derive(Clone, Debug)]
#[cfg_attr(any(test, feature = "serde"), derive(Serialize, Deserialize))]
#[cfg_attr(test, derive(PartialEq))]
pub struct New {
    /// Address of this operation in the program's operations vector.
    pub address: BytecodeAddress,
    /// Input type
    #[cfg_attr(any(test, feature = "serde"), serde(rename = "type"))]
    pub ty: NadaType,
    /// Source code info about this element.
    pub source_ref_index: SourceRefIndex,
}

impl Display for New {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} rty({}) = New", self.address, self.ty)
    }
}
source_info!(New);
typed_element!(New);
addressed_operation!(New);

/// Bytecode Unzip operation
#[derive(Clone, Debug)]
#[cfg_attr(any(test, feature = "serde"), derive(Serialize, Deserialize))]
#[cfg_attr(test, derive(PartialEq))]
pub struct Unzip {
    /// Array of tuples that will be unzipped
    pub target: BytecodeAddress,
    /// Address of this operation in the program's operations vector.
    pub address: BytecodeAddress,
    /// The output type of this operation as [`NadaType`], [`Tuple`]
    #[cfg_attr(any(test, feature = "serde"), serde(rename = "type"))]
    pub ty: NadaType,
    /// Source code info about this element.
    pub source_ref_index: SourceRefIndex,
}

#[cfg(test)]
pub mod tests {
    //! Utils for bytecode model testing

    use crate::models::{
        bytecode::{address_count, BytecodeAddress, ProgramBytecode},
        memory::AddressType,
    };
    use indexmap::IndexMap;
    use nada_type::NadaType;

    fn assert_memory_element(bytecode: &ProgramBytecode, address: BytecodeAddress, ty: &NadaType) {
        assert_eq!(ty, bytecode.memory_element_type(address).unwrap());
    }

    #[test]
    fn test_address_input_integer() {
        let mut bytecode = ProgramBytecode::default();
        let ty = NadaType::new_secret_unsigned_integer();
        let party_id = bytecode.create_new_party(String::from("dealer"));
        bytecode.create_new_input(String::from("Input"), party_id, ty.clone()).unwrap();

        assert_eq!(bytecode.memory.inputs.memory.len(), 1);
        assert_memory_element(&bytecode, BytecodeAddress(0, AddressType::Input), &ty);
    }

    #[test]
    fn test_address_input_array_integer() {
        let size = 5;
        let mut bytecode = ProgramBytecode::default();
        let inner_type = NadaType::new_secret_unsigned_integer();
        let ty = NadaType::new_array(inner_type.clone(), size).unwrap();
        let party_id = bytecode.create_new_party(String::from("dealer"));
        bytecode.create_new_input(String::from("Input"), party_id, ty.clone()).unwrap();

        assert_eq!(bytecode.memory.inputs.memory.len(), 6);
        assert_memory_element(&bytecode, BytecodeAddress(0, AddressType::Input), &ty);
        for address in 1..=size {
            let address = BytecodeAddress(address, AddressType::Input);
            assert_memory_element(&bytecode, address, &inner_type);
        }
    }

    #[test]
    fn test_address_input_array_array_integer() {
        let size = 5;
        let mut bytecode = ProgramBytecode::default();
        let primitive_type = NadaType::new_secret_unsigned_integer();
        let inner_type = NadaType::new_array(primitive_type.clone(), size).unwrap();
        let ty = NadaType::new_array(inner_type.clone(), size).unwrap();

        let party_id = bytecode.create_new_party(String::from("dealer"));
        bytecode.create_new_input(String::from("Input"), party_id, ty.clone()).unwrap();

        assert_eq!(bytecode.memory.inputs.memory.len(), 31);
        assert_memory_element(&bytecode, BytecodeAddress(0, AddressType::Input), &ty);
        for address in [1, 7, 13, 19, 25] {
            let array_address = BytecodeAddress(address, AddressType::Input);
            assert_memory_element(&bytecode, array_address, &inner_type);
            for inner_address in address + 1..=address + 5 {
                let inner_address = BytecodeAddress(inner_address, AddressType::Input);
                assert_memory_element(&bytecode, inner_address, &primitive_type);
            }
        }
    }

    #[test]
    fn test_address_input_tuple_integer() {
        let mut bytecode = ProgramBytecode::default();
        let primitive_type = NadaType::new_secret_unsigned_integer();
        let ty = NadaType::new_tuple(primitive_type.clone(), primitive_type.clone()).unwrap();

        let party_id = bytecode.create_new_party(String::from("dealer"));
        bytecode.create_new_input(String::from("Input"), party_id, ty.clone()).unwrap();

        assert_eq!(bytecode.memory.inputs.memory.len(), 3);
        assert_memory_element(&bytecode, BytecodeAddress(0usize, AddressType::Input), &ty);
        assert_memory_element(&bytecode, BytecodeAddress(1usize, AddressType::Input), &primitive_type);
        assert_memory_element(&bytecode, BytecodeAddress(2usize, AddressType::Input), &primitive_type);
    }

    #[test]
    fn test_address_input_array_tuple_integer() {
        let size = 5;
        let mut bytecode = ProgramBytecode::default();
        let primitive_type = NadaType::new_secret_unsigned_integer();
        let inner_type = NadaType::new_tuple(primitive_type.clone(), primitive_type.clone()).unwrap();
        let ty = NadaType::new_array(inner_type.clone(), size).unwrap();

        let party_id = bytecode.create_new_party(String::from("dealer"));
        bytecode.create_new_input(String::from("Input"), party_id, ty.clone()).unwrap();

        assert_eq!(bytecode.memory.inputs.memory.len(), 16);
        assert_memory_element(&bytecode, BytecodeAddress(0, AddressType::Input), &ty);
        for address in [1, 4, 7, 10, 13] {
            let address = BytecodeAddress(address, AddressType::Input);
            assert_memory_element(&bytecode, address, &inner_type);
            assert_memory_element(&bytecode, address.next().unwrap(), &primitive_type);
            assert_memory_element(&bytecode, address.advance(2).unwrap(), &primitive_type);
        }
    }

    #[test]
    fn test_address_input_array_tuple_array_integer() {
        let size = 5;
        let mut bytecode = ProgramBytecode::default();
        let primitive_type = NadaType::new_secret_unsigned_integer();
        let array_type = NadaType::new_array(primitive_type.clone(), size).unwrap();
        let tuple_type = NadaType::new_tuple(array_type.clone(), array_type.clone()).unwrap();
        let ty = NadaType::new_array(tuple_type.clone(), size).unwrap();

        let party_id = bytecode.create_new_party(String::from("dealer"));
        bytecode.create_new_input(String::from("Input"), party_id, ty.clone()).unwrap();

        assert_eq!(bytecode.memory.inputs.memory.len(), 66);
        assert_memory_element(&bytecode, BytecodeAddress(0, AddressType::Input), &ty);
        for address in [1, 14, 27, 40, 53] {
            let tuple_address = BytecodeAddress(address, AddressType::Input);
            assert_memory_element(&bytecode, tuple_address, &tuple_type);

            let left_array_address = tuple_address.next().unwrap();
            assert_memory_element(&bytecode, left_array_address, &array_type);
            assert_inner_type(&bytecode, left_array_address, 5, &primitive_type);

            let right_array_address = tuple_address.advance(7).unwrap();
            assert_memory_element(&bytecode, right_array_address, &array_type);
            assert_inner_type(&bytecode, right_array_address, 5, &primitive_type);
        }
    }

    #[test]
    fn test_address_input_ntuple_integer() {
        let mut bytecode = ProgramBytecode::default();
        let primitive_type = NadaType::new_secret_unsigned_integer();
        let ty = NadaType::new_n_tuple(vec![primitive_type.clone(), primitive_type.clone()]).unwrap();

        let party_id = bytecode.create_new_party(String::from("dealer"));
        bytecode.create_new_input(String::from("Input"), party_id, ty.clone()).unwrap();

        assert_eq!(bytecode.memory.inputs.memory.len(), 3);
        assert_memory_element(&bytecode, BytecodeAddress(0usize, AddressType::Input), &ty);
        assert_memory_element(&bytecode, BytecodeAddress(1usize, AddressType::Input), &primitive_type);
        assert_memory_element(&bytecode, BytecodeAddress(2usize, AddressType::Input), &primitive_type);
    }

    #[test]
    fn test_address_input_array_ntuple_integer() {
        let size = 5;
        let mut bytecode = ProgramBytecode::default();
        let primitive_type = NadaType::new_secret_unsigned_integer();
        let inner_type = NadaType::new_n_tuple(vec![primitive_type.clone(), primitive_type.clone()]).unwrap();
        let ty = NadaType::new_array(inner_type.clone(), size).unwrap();

        let party_id = bytecode.create_new_party(String::from("dealer"));
        bytecode.create_new_input(String::from("Input"), party_id, ty.clone()).unwrap();

        assert_eq!(bytecode.memory.inputs.memory.len(), 16);
        assert_memory_element(&bytecode, BytecodeAddress(0, AddressType::Input), &ty);
        for address in [1, 4, 7, 10, 13] {
            let address = BytecodeAddress(address, AddressType::Input);
            assert_memory_element(&bytecode, address, &inner_type);
            assert_memory_element(&bytecode, address.next().unwrap(), &primitive_type);
            assert_memory_element(&bytecode, address.advance(2).unwrap(), &primitive_type);
        }
    }

    #[test]
    fn test_address_input_array_ntuple_array_integer() {
        let size = 5;
        let mut bytecode = ProgramBytecode::default();
        let primitive_type = NadaType::new_secret_unsigned_integer();
        let array_type = NadaType::new_array(primitive_type.clone(), size).unwrap();
        let tuple_type = NadaType::new_n_tuple(vec![array_type.clone(), array_type.clone()]).unwrap();
        let ty = NadaType::new_array(tuple_type.clone(), size).unwrap();

        let party_id = bytecode.create_new_party(String::from("dealer"));
        bytecode.create_new_input(String::from("Input"), party_id, ty.clone()).unwrap();

        assert_eq!(bytecode.memory.inputs.memory.len(), 66);
        assert_memory_element(&bytecode, BytecodeAddress(0, AddressType::Input), &ty);
        for address in [1, 14, 27, 40, 53] {
            let tuple_address = BytecodeAddress(address, AddressType::Input);
            assert_memory_element(&bytecode, tuple_address, &tuple_type);

            let left_array_address = tuple_address.next().unwrap();
            assert_memory_element(&bytecode, left_array_address, &array_type);
            assert_inner_type(&bytecode, left_array_address, 5, &primitive_type);

            let right_array_address = tuple_address.advance(7).unwrap();
            assert_memory_element(&bytecode, right_array_address, &array_type);
            assert_inner_type(&bytecode, right_array_address, 5, &primitive_type);
        }
    }

    #[test]
    fn test_address_input_object_integer() {
        let mut bytecode = ProgramBytecode::default();
        let primitive_type = NadaType::new_secret_unsigned_integer();
        let ty = NadaType::new_object(IndexMap::from([
            ("a".to_string(), primitive_type.clone()),
            ("b".to_string(), primitive_type.clone()),
        ]))
        .unwrap();

        let party_id = bytecode.create_new_party(String::from("dealer"));
        bytecode.create_new_input(String::from("Input"), party_id, ty.clone()).unwrap();

        assert_eq!(bytecode.memory.inputs.memory.len(), 3);
        assert_memory_element(&bytecode, BytecodeAddress(0usize, AddressType::Input), &ty);
        assert_memory_element(&bytecode, BytecodeAddress(1usize, AddressType::Input), &primitive_type);
        assert_memory_element(&bytecode, BytecodeAddress(2usize, AddressType::Input), &primitive_type);
    }

    #[test]
    fn test_address_input_array_object_integer() {
        let size = 5;
        let mut bytecode = ProgramBytecode::default();
        let primitive_type = NadaType::new_secret_unsigned_integer();
        let inner_type = NadaType::new_object(IndexMap::from([
            ("a".to_string(), primitive_type.clone()),
            ("b".to_string(), primitive_type.clone()),
        ]))
        .unwrap();
        let ty = NadaType::new_array(inner_type.clone(), size).unwrap();

        let party_id = bytecode.create_new_party(String::from("dealer"));
        bytecode.create_new_input(String::from("Input"), party_id, ty.clone()).unwrap();

        assert_eq!(bytecode.memory.inputs.memory.len(), 16);
        assert_memory_element(&bytecode, BytecodeAddress(0, AddressType::Input), &ty);
        for address in [1, 4, 7, 10, 13] {
            let address = BytecodeAddress(address, AddressType::Input);
            assert_memory_element(&bytecode, address, &inner_type);
            assert_memory_element(&bytecode, address.next().unwrap(), &primitive_type);
            assert_memory_element(&bytecode, address.advance(2).unwrap(), &primitive_type);
        }
    }

    #[test]
    fn test_address_input_array_object_array_integer() {
        let size = 5;
        let mut bytecode = ProgramBytecode::default();
        let primitive_type = NadaType::new_secret_unsigned_integer();
        let array_type = NadaType::new_array(primitive_type.clone(), size).unwrap();
        let tuple_type = NadaType::new_object(IndexMap::from([
            ("a".to_string(), array_type.clone()),
            ("b".to_string(), array_type.clone()),
        ]))
        .unwrap();
        let ty = NadaType::new_array(tuple_type.clone(), size).unwrap();

        let party_id = bytecode.create_new_party(String::from("dealer"));
        bytecode.create_new_input(String::from("Input"), party_id, ty.clone()).unwrap();

        assert_eq!(bytecode.memory.inputs.memory.len(), 66);
        assert_memory_element(&bytecode, BytecodeAddress(0, AddressType::Input), &ty);
        for address in [1, 14, 27, 40, 53] {
            let tuple_address = BytecodeAddress(address, AddressType::Input);
            assert_memory_element(&bytecode, tuple_address, &tuple_type);

            let left_array_address = tuple_address.next().unwrap();
            assert_memory_element(&bytecode, left_array_address, &array_type);
            assert_inner_type(&bytecode, left_array_address, 5, &primitive_type);

            let right_array_address = tuple_address.advance(7).unwrap();
            assert_memory_element(&bytecode, right_array_address, &array_type);
            assert_inner_type(&bytecode, right_array_address, 5, &primitive_type);
        }
    }

    #[allow(clippy::arithmetic_side_effects)]
    fn assert_inner_type(
        bytecode: &ProgramBytecode,
        offset: BytecodeAddress,
        array_size: usize,
        inner_type: &NadaType,
    ) {
        let array_inner_addresses = (0..array_size).map(|inner_address| {
            let inner_offset = inner_address * address_count(inner_type).unwrap() + 1;
            offset.advance(inner_offset).unwrap()
        });
        for inner_address in array_inner_addresses {
            assert_memory_element(bytecode, inner_address, inner_type);
        }
    }
}
