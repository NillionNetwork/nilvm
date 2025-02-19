//! Execution engine memory implementation

use std::collections::HashMap;

use anyhow::{anyhow, Error};
use num_bigint::BigUint;
use thiserror::Error;

use jit_compiler::{
    models::{
        memory::{address_count, AddressCountError, AddressType},
        protocols::{
            memory::{ProtocolAddress, ProtocolMemoryError},
            InputMemoryAllocation, OutputMemoryScheme, Protocol,
        },
    },
    Program,
};
use math_lib::modular::{ModularNumber, Overflow, SafePrime};
use nada_value::{encrypted::Encrypted, NadaType, NadaValue, TypeError};

/// Memory value. This is an extension trait of NadaValue<Encrypted<T>> that provides to NadaValue
/// the capabilities that are needed during the runtime.
pub trait MemoryValue<T: SafePrime> {
    /// Creates a new NadaValue.
    fn new_memory_value(ty: NadaType, value: ModularNumber<T>) -> Result<NadaValue<Encrypted<T>>, Error>;
    /// Returns the underlying value
    fn try_into_value(self) -> Result<ModularNumber<T>, Error>;
}

impl<T: SafePrime> MemoryValue<T> for NadaValue<Encrypted<T>> {
    fn new_memory_value(ty: NadaType, value: ModularNumber<T>) -> Result<NadaValue<Encrypted<T>>, Error> {
        match ty {
            NadaType::Integer => Ok(NadaValue::new_integer(value)),
            NadaType::UnsignedInteger => Ok(NadaValue::new_unsigned_integer(value)),
            NadaType::Boolean => Ok(NadaValue::new_boolean(value)),
            NadaType::ShamirShareInteger => Ok(NadaValue::new_shamir_share_integer(value)),
            NadaType::ShamirShareUnsignedInteger => Ok(NadaValue::new_shamir_share_unsigned_integer(value)),
            NadaType::ShamirShareBoolean => Ok(NadaValue::new_shamir_share_boolean(value)),
            // These elements cannot be used as memory value.
            NadaType::Array { .. }
            | NadaType::Tuple { .. }
            | NadaType::NTuple { .. }
            | NadaType::Object { .. }
            // These elements cannot exist in the node
            | NadaType::SecretInteger
            | NadaType::SecretUnsignedInteger
            | NadaType::SecretBoolean
            | NadaType::SecretBlob
            | NadaType::EcdsaDigestMessage
            | NadaType::EcdsaPrivateKey
            | NadaType::EcdsaSignature
            | NadaType::EcdsaPublicKey
            | NadaType::StoreId
            | NadaType::EddsaPrivateKey
            | NadaType::EddsaPublicKey
            | NadaType::EddsaSignature
            | NadaType::EddsaMessage => Err(anyhow!("{ty} cannot be a memory value")),
        }
    }

    fn try_into_value(self) -> Result<ModularNumber<T>, Error> {
        match self {
            NadaValue::Integer(v)
            | NadaValue::UnsignedInteger(v)
            | NadaValue::Boolean(v)
            | NadaValue::ShamirShareInteger(v)
            | NadaValue::ShamirShareUnsignedInteger(v)
            | NadaValue::ShamirShareBoolean(v) => Ok(v),
            // The containers are flatten in memory and, they are not used during runtime.
            NadaValue::Array { .. }
            | NadaValue::Tuple { .. }
            | NadaValue::NTuple { .. }
            | NadaValue::Object { .. }
            // These elements cannot be created in runtime
            | NadaValue::SecretInteger(_)
            | NadaValue::SecretUnsignedInteger(_)
            | NadaValue::SecretBoolean(_)
            | NadaValue::SecretBlob(_)
            | NadaValue::EcdsaDigestMessage(_)
            | NadaValue::EcdsaPrivateKey(_)
            | NadaValue::EcdsaSignature(_)
            | NadaValue::EcdsaPublicKey(_)
            | NadaValue::StoreId(_)
            | NadaValue::EddsaPrivateKey(_)
            | NadaValue::EddsaPublicKey(_)
            | NadaValue::EddsaSignature(_)
            | NadaValue::EddsaMessage(_) => {
                Err(anyhow!("{} cannot be converted into a memory value", self.to_type()))
            }
        }
    }
}

/// Represents an available value in the memory
#[derive(Clone, Debug)]
struct ReadableValue<T: SafePrime> {
    /// Stored value
    value: NadaValue<Encrypted<T>>,
    /// Number of time that the value can be read
    reads: usize,
}

#[derive(Clone, Debug, Default)]
#[repr(u8)]
enum RuntimeMemoryElement<T: SafePrime> {
    /// Represent an empty memory position.
    #[default]
    Empty = 0,
    /// Represents a memory indirection. It contains the memory address to another memory element.
    Pointer(ProtocolAddress) = 1,
    /// Represents a runtime NadaValue
    Value(ReadableValue<T>) = 2,
    /// Represents the header, the beginning of a compound type, stores the NadaType associated with it
    Header(NadaType) = 3,
    /// Represents a value that has been freed from the memory and is not available anymore
    NotAvailable = 4,
}

#[derive(Default, Debug)]
pub(crate) struct RuntimeMemoryPool<T: SafePrime> {
    reads_table: HashMap<usize, usize>,
    memory_elements: Vec<RuntimeMemoryElement<T>>,
}

impl<T: SafePrime> RuntimeMemoryPool<T> {
    fn new(reads_table: HashMap<usize, usize>, size: usize) -> Self {
        Self { reads_table, memory_elements: vec![RuntimeMemoryElement::Empty; size] }
    }

    fn reads(&mut self, address: &ProtocolAddress) -> usize {
        self.reads_table.remove(&address.0).unwrap_or_default()
    }

    fn collect_addresses<'a, I>(
        &self,
        mut pointer_address: ProtocolAddress,
        types: I,
        address_type: &mut Vec<(ProtocolAddress, &'a NadaType)>,
    ) -> Result<(), RuntimeMemoryError>
    where
        I: IntoIterator<Item = &'a NadaType>,
    {
        for ty in types {
            let element_address = self.resolve_pointer(pointer_address)?;
            address_type.push((element_address, ty));
            pointer_address = pointer_address.next()?;
        }
        Ok(())
    }

    // The first address is reserved. It indicates that we are storing a compound type.
    fn collect_values(
        result: &mut Vec<NadaValue<Encrypted<T>>>,
        count: usize,
    ) -> Result<Vec<NadaValue<Encrypted<T>>>, RuntimeMemoryError> {
        let mut values = Vec::with_capacity(count);
        for _ in 0..count {
            if let Some(value) = result.pop() {
                values.push(value);
            } else {
                return Err(RuntimeMemoryError::NotEnoughValues);
            }
        }
        values.reverse();
        Ok(values)
    }

    /// Read a value from the memory.
    fn read_value(&mut self, address: ProtocolAddress) -> Result<NadaValue<Encrypted<T>>, RuntimeMemoryError> {
        let mut flattened_values = vec![];
        let mut flattened_types = vec![];
        let ty = self.runtime_memory_type(address)?;
        let mut address_type = vec![(address, &ty)];
        while let Some((address, ty)) = address_type.pop() {
            match ty {
                NadaType::Integer
                | NadaType::UnsignedInteger
                | NadaType::Boolean
                | NadaType::EcdsaDigestMessage
                | NadaType::EcdsaSignature
                | NadaType::EcdsaPrivateKey
                | NadaType::EcdsaPublicKey
                | NadaType::StoreId
                | NadaType::ShamirShareInteger
                | NadaType::ShamirShareUnsignedInteger
                | NadaType::ShamirShareBoolean
                | NadaType::EddsaPrivateKey
                | NadaType::EddsaPublicKey
                | NadaType::EddsaSignature
                | NadaType::EddsaMessage => flattened_values.push(self.read_primitive_value(address)?),
                NadaType::Array { inner_type, size } if inner_type.is_primitive() => {
                    flattened_values.push(self.read_primitive_array(address, *size)?);
                }
                NadaType::Array { inner_type, size } => {
                    // First address is reserved in the memory. It indicates the beginning of the array
                    let inner_type_ref = inner_type.as_ref();
                    let types = std::iter::repeat(inner_type_ref).take(*size);
                    self.collect_addresses(address.next()?, types, &mut address_type)?;
                }
                NadaType::Tuple { left_type, right_type } => {
                    let types = &[&**left_type, &**right_type];
                    self.collect_addresses(address.next()?, types.iter().copied(), &mut address_type)?;
                }
                NadaType::NTuple { types } => {
                    self.collect_addresses(address.next()?, types.iter(), &mut address_type)?;
                }
                NadaType::Object { types } => {
                    self.collect_addresses(address.next()?, types.values(), &mut address_type)?;
                }
                NadaType::SecretInteger
                | NadaType::SecretUnsignedInteger
                | NadaType::SecretBoolean
                | NadaType::SecretBlob => {
                    return Err(RuntimeMemoryError::IllegalType(ty.clone()));
                }
            }
            flattened_types.push(ty)
        }
        let mut result = vec![];
        while let Some(ty) = flattened_types.pop() {
            match ty {
                NadaType::Integer
                | NadaType::UnsignedInteger
                | NadaType::Boolean
                | NadaType::EcdsaDigestMessage
                | NadaType::EcdsaSignature
                | NadaType::EcdsaPrivateKey
                | NadaType::EcdsaPublicKey
                | NadaType::StoreId
                | NadaType::ShamirShareInteger
                | NadaType::ShamirShareUnsignedInteger
                | NadaType::ShamirShareBoolean
                | NadaType::EddsaPrivateKey
                | NadaType::EddsaPublicKey
                | NadaType::EddsaSignature
                | NadaType::EddsaMessage => result.extend(flattened_values.pop()),
                NadaType::Array { inner_type, .. } if inner_type.is_primitive() => {
                    result.extend(flattened_values.pop())
                }
                NadaType::Array { size, inner_type } => {
                    let values = Self::collect_values(&mut result, *size)?;
                    result.push(NadaValue::new_array(inner_type.as_ref().clone(), values)?);
                }
                NadaType::Tuple { .. } => {
                    let values = Self::collect_values(&mut result, 2)?;
                    let [left, right] = values.try_into().map_err(|_| RuntimeMemoryError::NotEnoughValues)?;
                    result.push(NadaValue::new_tuple(left, right)?)
                }
                NadaType::NTuple { types } => {
                    let values = Self::collect_values(&mut result, types.len())?;
                    result.push(NadaValue::new_n_tuple(values)?);
                }
                NadaType::Object { types } => {
                    let mut values = Vec::with_capacity(types.len());
                    for _ in 0..types.len() {
                        values.extend(result.pop());
                    }

                    if values.len() < types.len() {
                        return Err(RuntimeMemoryError::NotEnoughValues);
                    }
                    values.reverse();
                    result.push(NadaValue::new_object(types.keys().cloned().zip(values.into_iter()).collect())?);
                }
                // These elements cannot exist in the node
                NadaType::SecretInteger
                | NadaType::SecretUnsignedInteger
                | NadaType::SecretBoolean
                | NadaType::SecretBlob => {
                    return Err(RuntimeMemoryError::IllegalType(ty.clone()));
                }
            }
        }
        result.pop().ok_or(RuntimeMemoryError::NotEnoughValues)
    }

    /// Get a primitive value from the runtime memory.
    fn read_primitive_value(
        &mut self,
        address: ProtocolAddress,
    ) -> Result<NadaValue<Encrypted<T>>, RuntimeMemoryError> {
        let element = self.memory_elements.get_mut(address.0).ok_or(RuntimeMemoryError::OutOfMemory(address))?;
        match std::mem::take(element) {
            RuntimeMemoryElement::Empty
            | RuntimeMemoryElement::Header(_)
            // We avoid the indirections to the primitive value.
            | RuntimeMemoryElement::Pointer(_) => Err(RuntimeMemoryError::IllegalMemoryAccess),
            | RuntimeMemoryElement::NotAvailable => Err(RuntimeMemoryError::NotAvailableValue(address)),
            | RuntimeMemoryElement::Value(readable_value) if readable_value.reads == 0 => {
                // This shouldn't happen, but in that case, it's manage as a RuntimeMemoryElement::NotAvailable
                Err(RuntimeMemoryError::NotAvailableValue(address))
            }
            | RuntimeMemoryElement::Value(mut readable_value) if readable_value.reads > 1 => {
                let value = readable_value.value.clone();
                readable_value.reads = readable_value.reads.wrapping_sub(1);
                *element = RuntimeMemoryElement::Value(readable_value);
                Ok(value)
            }
            | RuntimeMemoryElement::Value(readable_value) => {
                *element = RuntimeMemoryElement::NotAvailable;
                Ok(readable_value.value)
            }
        }
    }

    /// Read a primitive array from memory.
    #[allow(clippy::indexing_slicing)]
    fn read_primitive_array(
        &mut self,
        address: ProtocolAddress,
        size: usize,
    ) -> Result<NadaValue<Encrypted<T>>, RuntimeMemoryError> {
        let first_address = address.next()?;
        let last_address = address.advance(size)?;
        if last_address.0 >= self.memory_elements.len() {
            return Err(RuntimeMemoryError::OutOfMemory(last_address));
        }
        let mut inner_values = Vec::with_capacity(size);
        for element in &mut self.memory_elements[first_address.0..=last_address.0] {
            match element {
                RuntimeMemoryElement::Empty
                | RuntimeMemoryElement::Header(_)
                // The primitive arrays contain primitive value always, so we don't find a pointer. 
                // If we find one, it is an error because we are avoiding indirections to primitive
                // values.
                | RuntimeMemoryElement::Pointer(_) => {
                    return Err(RuntimeMemoryError::IllegalMemoryAccess);
                }
                | RuntimeMemoryElement::NotAvailable => return Err(RuntimeMemoryError::NotAvailableValue(address)),
                | RuntimeMemoryElement::Value(readable_value) if readable_value.reads == 0 => {
                    // This shouldn't happen, but in that case, it's manage as a RuntimeMemoryElement::NotAvailable
                    return Err(RuntimeMemoryError::NotAvailableValue(address))
                }
                RuntimeMemoryElement::Value(readable_value) if readable_value.reads > 1 => {
                    readable_value.reads = readable_value.reads.wrapping_add(1);
                    inner_values.push(readable_value.value.clone());
                }
                RuntimeMemoryElement::Value(readable_value) => {
                    inner_values.push(readable_value.value.clone());
                    std::mem::swap(element, &mut RuntimeMemoryElement::NotAvailable);
                }
            }
        }
        Ok(NadaValue::new_array_non_empty(inner_values)?)
    }

    /// Return the type of the element stored in the runtime memory.
    fn runtime_memory_type(&mut self, mut address: ProtocolAddress) -> Result<NadaType, RuntimeMemoryError> {
        loop {
            match self.memory_elements.get(address.0).ok_or(RuntimeMemoryError::OutOfMemory(address))? {
                RuntimeMemoryElement::Empty => return Err(RuntimeMemoryError::IllegalMemoryAccess),
                RuntimeMemoryElement::NotAvailable => return Err(RuntimeMemoryError::NotAvailableValue(address)),
                RuntimeMemoryElement::Pointer(ptr) => address = *ptr,
                RuntimeMemoryElement::Value(readable_value) => {
                    return Ok(readable_value.value.to_type());
                }
                RuntimeMemoryElement::Header(ty) => {
                    return Ok(ty.clone());
                }
            }
        }
    }

    /// Stores a runtime element into the memory.
    fn store_element(
        &mut self,
        address: ProtocolAddress,
        element: RuntimeMemoryElement<T>,
    ) -> Result<(), RuntimeMemoryError> {
        *self.memory_elements.get_mut(address.0).ok_or_else(|| RuntimeMemoryError::OutOfMemory(address))? = element;
        Ok(())
    }

    /// Stores a new memory element.
    ///
    /// Stores a new protocol memory address and inserts the element in the new address.
    fn store(
        &mut self,
        mut address: ProtocolAddress,
        value: NadaValue<Encrypted<T>>,
    ) -> Result<(), RuntimeMemoryError> {
        let mut inner_values = vec![value];
        while let Some(value) = inner_values.pop() {
            let ty = value.to_type();
            let memory_element = match value {
                NadaValue::Integer(_)
                | NadaValue::UnsignedInteger(_)
                | NadaValue::Boolean(_)
                | NadaValue::ShamirShareInteger(_)
                | NadaValue::ShamirShareUnsignedInteger(_)
                | NadaValue::EcdsaDigestMessage(_)
                | NadaValue::EcdsaPrivateKey(_)
                | NadaValue::EcdsaSignature(_)
                | NadaValue::EcdsaPublicKey(_)
                | NadaValue::StoreId(_)
                | NadaValue::ShamirShareBoolean(_)
                | NadaValue::EddsaPrivateKey(_)
                | NadaValue::EddsaPublicKey(_)
                | NadaValue::EddsaSignature(_)
                | NadaValue::EddsaMessage(_) => {
                    RuntimeMemoryElement::Value(ReadableValue { value, reads: self.reads(&address) })
                }
                NadaValue::Array { mut values, .. } | NadaValue::NTuple { mut values } => {
                    values.reverse();
                    inner_values.extend(values);
                    RuntimeMemoryElement::Header(ty)
                }
                NadaValue::Tuple { left, right } => {
                    inner_values.push(*right);
                    inner_values.push(*left);
                    RuntimeMemoryElement::Header(ty)
                }
                NadaValue::Object { mut values } => {
                    values.reverse();
                    inner_values.extend(values.values().cloned());
                    RuntimeMemoryElement::Header(ty)
                }
                NadaValue::SecretInteger(_)
                | NadaValue::SecretUnsignedInteger(_)
                | NadaValue::SecretBoolean(_)
                | NadaValue::SecretBlob(_) => return Err(RuntimeMemoryError::IllegalType(ty)),
            };
            self.store_element(address, memory_element)?;
            address = address.next()?;
        }
        Ok(())
    }

    fn store_header(&mut self, address: ProtocolAddress, ty: NadaType) {
        self.memory_elements.insert(address.0, RuntimeMemoryElement::Header(ty));
    }

    /// Allocate a pointer to the provided memory address in the top of the heap
    fn create_ptr(
        &mut self,
        pointer_address: ProtocolAddress,
        ref_address: ProtocolAddress,
    ) -> Result<(), RuntimeMemoryError> {
        if let Ok(value) = self.read_primitive_value(ref_address) {
            // If the pointed element is a value, we store directly a copy of the value to avoid
            // indirections to primitive values.
            self.store(pointer_address, value)
        } else {
            self.memory_elements.insert(pointer_address.0, RuntimeMemoryElement::Pointer(ref_address));
            Ok(())
        }
    }

    /// Return the address that points to a value. If the allocated element is a pointer, this returns
    /// the pointed address. Otherwise, the address points to a value
    fn resolve_pointer(&self, address: ProtocolAddress) -> Result<ProtocolAddress, RuntimeMemoryError> {
        match self.memory_elements.get(address.0) {
            Some(RuntimeMemoryElement::Pointer(incoming_address)) => Ok(*incoming_address),
            Some(_) => Ok(address),
            None => Err(RuntimeMemoryError::OutOfMemory(address)),
        }
    }
}

#[derive(Debug)]
#[cfg_attr(test, derive(Default))]
/// This memory is used by the execution engine to allocate the protocol's resultant values.
/// The memory is split in two different pools:
/// - Literals: Memory where are allocated the literals are used by the program.
/// - Heap: Memory where are stored the values that are used during a program execution.
///
/// Besides, this memory contains a scheme as how the outputs should be retrieved when a program
/// execution has finished.
pub struct RuntimeMemory<T: SafePrime> {
    pub(crate) literals: Vec<NadaValue<Encrypted<T>>>,
    pub(crate) heap: RuntimeMemoryPool<T>,
    pub(crate) output_memory_scheme: OutputMemoryScheme,
}

impl<T: SafePrime> RuntimeMemory<T> {
    /// Creates a new instance of the RuntimeMemory.
    pub fn new<P: Protocol>(
        program: &Program<P>,
        values: HashMap<String, NadaValue<Encrypted<T>>>,
    ) -> Result<Self, RuntimeMemoryError> {
        let reads_table = program
            .body
            .reads_table
            .iter()
            .filter(|(address, _)| matches!(address.1, AddressType::Heap) || matches!(address.1, AddressType::Input))
            .map(|(address, reads)| (address.0, *reads))
            .collect();
        let mut memory = Self {
            literals: Self::initialize_literals(program)?,
            heap: RuntimeMemoryPool::new(reads_table, program.body.memory_size()),
            output_memory_scheme: program.body.output_memory_scheme.clone(),
        };
        memory.initialize_heap(program, values)?;
        Ok(memory)
    }

    /// Check that the memory elements for an input match the expected by the program.
    fn check_input_allocation(
        memory_allocation: &InputMemoryAllocation,
        input_value: &NadaValue<Encrypted<T>>,
    ) -> Result<(), RuntimeMemoryError> {
        let required_addresses = address_count(&input_value.to_type())?;
        if memory_allocation.sizeof < required_addresses {
            return Err(RuntimeMemoryError::NotEnoughReservedMemory(memory_allocation.sizeof, required_addresses));
        }
        if memory_allocation.sizeof > required_addresses {
            return Err(RuntimeMemoryError::MissingElements(memory_allocation.sizeof, required_addresses));
        }
        Ok(())
    }

    /// Initializes the heap. The memory elements distribution that is generated from the inputs
    /// must match the expected memory distribution by the program.
    fn initialize_heap<P: Protocol>(
        &mut self,
        program: &Program<P>,
        mut values: HashMap<String, NadaValue<Encrypted<T>>>,
    ) -> Result<(), RuntimeMemoryError> {
        let mut next_address = ProtocolAddress::default();
        for memory_allocation in program.body.input_memory_scheme.values() {
            let input_name = &memory_allocation.input;
            let value =
                values.remove(input_name).ok_or_else(|| RuntimeMemoryError::InputNotFound(input_name.to_string()))?;

            // Checks if the memory elements distribution matches the expected distribution
            Self::check_input_allocation(memory_allocation, &value)?;
            self.heap.store(next_address, value)?;
            next_address = next_address.advance(memory_allocation.sizeof)?;
        }
        Ok(())
    }

    /// Initializes the memory of literals. The literals are provided by the program directly.
    fn initialize_literals<P: Protocol>(
        program: &Program<P>,
    ) -> Result<Vec<NadaValue<Encrypted<T>>>, RuntimeMemoryError> {
        let mut literals = Vec::new();
        for literal in &program.body.literals {
            let literal = match &literal.value {
                NadaValue::Integer(value) => NadaValue::new_integer(ModularNumber::try_from(value)?),
                NadaValue::UnsignedInteger(value) => NadaValue::new_unsigned_integer(ModularNumber::try_from(value)?),
                NadaValue::Boolean(value) => {
                    let value = ModularNumber::try_from(&BigUint::from(*value as u32))?;
                    NadaValue::new_boolean(value)
                }
                value => return Err(RuntimeMemoryError::NonLiteralAllocation(value.to_type()))?,
            };
            literals.push(literal);
        }
        Ok(literals)
    }

    /// Stores a memory element into the memory
    pub fn store(
        &mut self,
        address: ProtocolAddress,
        memory_element: NadaValue<Encrypted<T>>,
    ) -> Result<(), RuntimeMemoryError> {
        self.heap.store(address, memory_element)
    }

    /// Stores the header of a compound type
    pub fn store_header(&mut self, address: ProtocolAddress, ty: NadaType) {
        self.heap.store_header(address, ty)
    }

    /// Allocate a pointer to the provided memory address
    pub fn create_ptr(
        &mut self,
        pointer_address: ProtocolAddress,
        ref_address: ProtocolAddress,
    ) -> Result<(), RuntimeMemoryError> {
        self.heap.create_ptr(pointer_address, ref_address)
    }

    /// Reads the value that is stored in a memory address.
    pub fn read_value(&mut self, address: ProtocolAddress) -> Result<NadaValue<Encrypted<T>>, RuntimeMemoryError> {
        // Check the memory type the address refers to. If the memory address refers to the output
        // memory it returns a failure.
        match address.1 {
            AddressType::Heap => self.heap.read_value(address),
            // This is a quirkiness of the current model. The inputs are stored in the heap.
            AddressType::Input => self.heap.read_value(address.as_heap()),
            AddressType::Literals => {
                let index: usize = address.into();
                self.literals.get(index).cloned().ok_or(RuntimeMemoryError::OutOfMemory(address))
            }
            AddressType::Output => Err(RuntimeMemoryError::Unimplemented("output accessors".to_string()))?,
        }
    }

    /// Reads the type that is stored in a memory address
    pub fn runtime_memory_type(&mut self, address: ProtocolAddress) -> Result<NadaType, RuntimeMemoryError> {
        match address.1 {
            AddressType::Heap => self.heap.runtime_memory_type(address),
            // This is a quirkiness of the current model. The inputs are stored in the heap.
            AddressType::Input => self.heap.runtime_memory_type(ProtocolAddress(address.0, AddressType::Heap)),
            AddressType::Literals => {
                let index: usize = address.into();
                Ok(self.literals.get(index).cloned().ok_or(RuntimeMemoryError::OutOfMemory(address))?.to_type())
            }
            AddressType::Output => Err(RuntimeMemoryError::Unimplemented("output accessors".to_string()))?,
        }
    }
}

#[derive(Error, Debug)]
/// Errors throw during the element allocation in runtime memory
pub enum RuntimeMemoryError {
    /// This is thrown when the size is lesser than expected
    #[error("missing memory elements: expected {0} elements, found {1} elements")]
    MissingElements(usize, usize),

    /// This is thrown when the size is greater than expected
    #[error("not enough reserved memory: expected {0} elements, found {1} elements")]
    NotEnoughReservedMemory(usize, usize),

    /// This is thrown when we try to store a literal, but the type is not supported.
    #[error("literal cannot be allocated: {0} is not supported")]
    NonLiteralAllocation(NadaType),

    /// It is thrown when we try to read a reserved memory address
    #[error("illegal memory access")]
    IllegalMemoryAccess,

    /// It is thrown when we try to read a reserved memory address
    #[error("value allocated in {0} has been read too many times and is not available anymore")]
    NotAvailableValue(ProtocolAddress),

    /// An input hasn't been provided
    #[error("input not found: {0}")]
    InputNotFound(String),

    /// Address is out of memory
    #[error("out of memory: {0:?}")]
    OutOfMemory(ProtocolAddress),

    /// Not implemented.
    #[error("not implemented: {0}")]
    Unimplemented(String),

    /// Address count error
    #[error("inner address calculation: {0}")]
    AddressCountError(#[from] AddressCountError),

    /// Overflow error. It can be thrown during the input is translated into a memory element
    #[error(transparent)]
    Overflow(#[from] Overflow),

    /// This error is thrown when a value is not fully allocated into the memory.
    #[error("value is not fully allocated into the memory")]
    NotEnoughValues,

    /// Memory overflow
    #[error("memory overflow: {0}")]
    MemoryOverflow(#[from] ProtocolMemoryError),

    /// This error is thrown during array creation
    #[error("array value access error: {0}")]
    ArrayCreation(#[from] TypeError),

    /// This error is thrown when a value with an unsupported type is stored or read
    #[error("illegal type: {0} cannot be stored in the runtime memory")]
    IllegalType(NadaType),
}

#[cfg(test)]
mod tests {
    use crate::vm::memory::{RuntimeMemory, RuntimeMemoryElement, RuntimeMemoryError, RuntimeMemoryPool};
    use anyhow::Error;
    use indexmap::IndexMap;
    use jit_compiler::models::protocols::{memory::ProtocolAddress, InputMemoryAllocation};
    use math_lib::modular::{ModularNumber, SafePrime, U64SafePrime};
    use nada_value::{encrypted::Encrypted, NadaValue};
    use std::collections::HashMap;

    type Prime = U64SafePrime;

    // For these tests the value of the shares are not important
    fn random_share<T: SafePrime>() -> NadaValue<Encrypted<T>> {
        NadaValue::new_shamir_share_integer(ModularNumber::gen_random())
    }

    fn public_variable<T: SafePrime>(value: u64) -> NadaValue<Encrypted<T>> {
        NadaValue::Integer(ModularNumber::from_u64(value))
    }

    #[test]
    fn store_public() -> Result<(), Error> {
        let mut memory = RuntimeMemoryPool::<Prime>::new(HashMap::new(), 1);
        memory.store(ProtocolAddress::default(), public_variable(5))?;
        assert_eq!(memory.memory_elements.len(), 1);
        assert!(matches!(memory.memory_elements.first().unwrap(), RuntimeMemoryElement::Value(_)));
        Ok(())
    }

    #[test]
    fn store_share() -> Result<(), Error> {
        let mut memory = RuntimeMemoryPool::<Prime>::new(HashMap::new(), 1);
        memory.store(ProtocolAddress::default(), random_share())?;
        assert_eq!(memory.memory_elements.len(), 1);
        assert!(matches!(memory.memory_elements.first().unwrap(), RuntimeMemoryElement::Value(_)));
        Ok(())
    }

    #[test]
    fn store_array_of_shares() -> Result<(), Error> {
        let mut memory = RuntimeMemoryPool::<Prime>::new(HashMap::new(), 6);
        memory.store(ProtocolAddress::default(), NadaValue::new_array_non_empty(vec![random_share(); 5])?)?;
        assert_eq!(memory.memory_elements.len(), 6);
        assert!(matches!(memory.memory_elements.first().unwrap(), RuntimeMemoryElement::Header(_)));
        for index in 1..=5 {
            assert!(matches!(memory.memory_elements.get(index).unwrap(), RuntimeMemoryElement::Value(_)));
        }
        Ok(())
    }

    #[test]
    fn reuse_share() -> Result<(), Error> {
        let address = ProtocolAddress::default();
        let mut reads_table = HashMap::new();
        reads_table.insert(address.0, 2);
        let mut memory = RuntimeMemoryPool::<Prime>::new(reads_table, 1);
        memory.store(address, random_share())?;
        assert!(memory.read_value(address).is_ok());
        assert!(memory.read_value(address).is_ok());
        Ok(())
    }

    #[test]
    fn reuse_share_error() -> Result<(), Error> {
        let address = ProtocolAddress::default();
        let mut reads_table = HashMap::new();
        reads_table.insert(address.0, 1);
        let mut memory = RuntimeMemoryPool::<Prime>::new(reads_table, 1);
        memory.store(address, random_share())?;
        assert!(memory.read_value(address).is_ok());
        assert!(memory.read_value(address).is_err());
        Ok(())
    }

    #[test]
    fn error_missing_share() -> Result<(), Error> {
        let input = String::from("my_var");
        let value = random_share::<Prime>();
        let memory_allocation = InputMemoryAllocation { input, sizeof: 0 };
        let result = RuntimeMemory::check_input_allocation(&memory_allocation, &value);
        assert!(matches!(result, Err(RuntimeMemoryError::NotEnoughReservedMemory(0, 1))));
        Ok(())
    }

    #[test]
    fn error_no_enough_shares() {
        let input = String::from("my_var");
        let value = random_share::<Prime>();
        let memory_allocation = InputMemoryAllocation { input, sizeof: 2 };
        let result = RuntimeMemory::check_input_allocation(&memory_allocation, &value);
        assert!(matches!(result, Err(RuntimeMemoryError::MissingElements(2, 1))));
    }

    #[test]
    fn reuse_public_variables() -> Result<(), Error> {
        let address = ProtocolAddress::default();
        let mut reads_table = HashMap::new();
        reads_table.insert(address.0, 2);
        let mut memory = RuntimeMemoryPool::<Prime>::new(reads_table, 1);
        memory.store(address, public_variable(5))?;
        assert!(memory.read_value(address).is_ok());
        assert!(memory.read_value(address).is_ok());
        Ok(())
    }

    #[test]
    fn reuse_public_variables_error() -> Result<(), Error> {
        let address = ProtocolAddress::default();
        let mut reads_table = HashMap::new();
        reads_table.insert(address.0, 1);
        let mut memory = RuntimeMemoryPool::<Prime>::new(reads_table, 1);
        memory.store(address, public_variable(5))?;
        assert!(memory.read_value(address).is_ok());
        assert!(memory.read_value(address).is_err());
        Ok(())
    }

    #[test]
    fn store_array_read_memory_type() -> Result<(), Error> {
        let inner_values = vec![random_share::<Prime>(); 5];
        let value = NadaValue::new_array_non_empty(inner_values)?;
        let mut memory = RuntimeMemoryPool::new(HashMap::new(), 6);
        memory.store(ProtocolAddress::default(), value)?;
        assert_eq!(memory.memory_elements.len(), 6);
        assert!(matches!(memory.memory_elements.first().unwrap(), RuntimeMemoryElement::Header(_)));
        for index in 1..=5 {
            assert!(matches!(memory.memory_elements.get(index).unwrap(), RuntimeMemoryElement::Value(_)));
        }
        Ok(())
    }

    #[allow(clippy::arithmetic_side_effects)]
    #[test]
    fn store_n_dimensional_array_of_shares() -> Result<(), Error> {
        let inner_array = vec![random_share::<Prime>(); 2];
        let value = NadaValue::new_array_non_empty(vec![NadaValue::new_array_non_empty(inner_array.clone())?; 5])?;
        let mut memory = RuntimeMemoryPool::<Prime>::new(HashMap::new(), 16);
        memory.store(ProtocolAddress::default(), value)?;
        assert_eq!(memory.memory_elements.len(), 16);

        let mut index = 0usize;
        assert!(matches!(memory.memory_elements.get(index).unwrap(), RuntimeMemoryElement::Header(_)));
        index += 1;
        for _ in 0..5 {
            assert!(matches!(memory.memory_elements.get(index).unwrap(), RuntimeMemoryElement::Header(_)));
            index += 1;
            for _ in 0..2 {
                assert!(matches!(memory.memory_elements.get(index).unwrap(), RuntimeMemoryElement::Value(_)));
                index += 1;
            }
        }
        Ok(())
    }

    #[test]
    fn store_tuple_of_shares() -> Result<(), Error> {
        let value = NadaValue::new_tuple(random_share::<Prime>(), random_share::<Prime>())?;
        let mut memory = RuntimeMemoryPool::<Prime>::new(HashMap::new(), 3);
        memory.store(ProtocolAddress::default(), value)?;
        assert_eq!(memory.memory_elements.len(), 3);
        assert!(matches!(memory.memory_elements.first().unwrap(), RuntimeMemoryElement::Header(_)));
        assert!(matches!(memory.memory_elements.get(1).unwrap(), RuntimeMemoryElement::Value(_)));
        assert!(matches!(memory.memory_elements.get(2).unwrap(), RuntimeMemoryElement::Value(_)));
        Ok(())
    }

    #[test]
    fn store_ntuple() -> Result<(), Error> {
        let value = NadaValue::new_n_tuple(vec![public_variable(5), public_variable(6)])?;
        let mut memory = RuntimeMemoryPool::<Prime>::new(HashMap::new(), 3);
        memory.store(ProtocolAddress::default(), value)?;
        assert_eq!(memory.memory_elements.len(), 3);
        assert!(matches!(memory.memory_elements.first().unwrap(), RuntimeMemoryElement::Header(_)));
        assert!(matches!(memory.memory_elements.get(1).unwrap(), RuntimeMemoryElement::Value(_)));
        assert!(matches!(memory.memory_elements.get(2).unwrap(), RuntimeMemoryElement::Value(_)));
        Ok(())
    }

    #[test]
    fn store_object() -> Result<(), Error> {
        let value = NadaValue::new_object(IndexMap::from([
            ("a".to_string(), public_variable(5)),
            ("b".to_string(), public_variable(6)),
        ]))?;
        let mut memory = RuntimeMemoryPool::<Prime>::new(HashMap::new(), 3);
        memory.store(ProtocolAddress::default(), value)?;
        assert_eq!(memory.memory_elements.len(), 3);
        assert!(matches!(memory.memory_elements.first().unwrap(), RuntimeMemoryElement::Header(_)));
        assert!(matches!(memory.memory_elements.get(1).unwrap(), RuntimeMemoryElement::Value(_)));
        assert!(matches!(memory.memory_elements.get(2).unwrap(), RuntimeMemoryElement::Value(_)));
        Ok(())
    }
}
