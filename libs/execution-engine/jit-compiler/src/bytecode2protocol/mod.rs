//! This crate implements the bytecode to protocol transformation.
pub mod errors;
mod utils;

use crate::{
    bytecode2protocol::errors::Bytecode2ProtocolError,
    models::{
        bytecode::{
            memory::BytecodeAddress, Addition, AddressedElement, Cast, Division, EcdsaSign, EddsaSign, Equals, IfElse,
            InnerProduct, LeftShift, LessThan, LiteralRef, Load, Modulo, Multiplication, Not, Operation, Power,
            ProgramBytecode, PublicKeyDerive, PublicOutputEquality, Random, Reveal, RightShift, Subtraction, TruncPr,
        },
        memory::{address_count, AddressType},
        protocols::{memory::ProtocolAddress, InputMemoryAllocation, OutputMemoryAllocation, Protocol, ProtocolsModel},
    },
};
use nada_compiler_backend::mir::{NamedElement, TypedElement};
use nada_type::{NadaPrimitiveType, NadaType, NadaTypeMetadata, Shape};
use std::collections::HashMap;

/// Provides protocol addresses. It keeps a count of the used address and increases the counter
/// when any protocol consumes new addresses.
#[derive(Default)]
pub(crate) struct ProtocolAddressProvider {
    /// Pointer to the first available memory address
    next_available_address: ProtocolAddress,
}

impl ProtocolAddressProvider {
    /// Returns the next available address. Additionally, the pointer to the next available address
    /// is moved as many addresses as necessary to store the value in the memory.
    pub(crate) fn next_available_address(&mut self, ty: &NadaType) -> Result<ProtocolAddress, Bytecode2ProtocolError> {
        let address = self.next_available_address;
        self.next_available_address = self.next_available_address.next()?;

        // When the protocol output type is a compound type, we have to reserve an address
        // for every inner element
        self.next_available_address = match ty {
            NadaType::Array { size, .. } => self.next_available_address.advance(*size)?,
            NadaType::Tuple { .. } => self.next_available_address.advance(2)?,
            ty if ty.is_primitive() => self.next_available_address,
            _ => {
                return Err(Bytecode2ProtocolError::UnsupportedCompoundType);
            }
        };
        Ok(address)
    }
}

/// Context required by the Bytecode to Protocol transformer.
pub struct Bytecode2ProtocolContext<'b, P: Protocol, F: ProtocolFactory<P>> {
    /// This factory will be used to create the protocols from the operations
    protocol_factory: F,
    /// Protocols address provider
    address_provider: ProtocolAddressProvider,
    /// Input bytecode of the transformation.
    pub bytecode: &'b ProgramBytecode,
    /// Resultant Protocols model
    program: ProtocolsModel<P>,
    /// Table that keeps the translations of a bytecode memory address into a protocol memory address.
    /// A bytecode memory address can be translated into different protocols depending on how other
    /// protocols consume the value.
    /// In order to avoid duplicates, this map keeps a trace between the context as how the result
    /// of the protocol are consumed (this is expected output type of the protocol).
    links: HashMap<BytecodeAddress, HashMap<NadaType, ProtocolAddress>>,
    /// Table that keeps the trace between the protocol address and the bytecode address of the operation
    /// from it was created.
    inverse_links: HashMap<ProtocolAddress, BytecodeAddress>,
    memory_indirections: HashMap<BytecodeAddress, BytecodeAddress>,
}

impl<'b, P: Protocol, F: ProtocolFactory<P>> Bytecode2ProtocolContext<'b, P, F> {
    /// Creates a new bytecode to protocol transformation context
    pub(crate) fn new(protocol_factory: F, bytecode: &'b ProgramBytecode) -> Self {
        Self {
            protocol_factory,
            address_provider: ProtocolAddressProvider::default(),
            bytecode,
            program: ProtocolsModel::new(bytecode.source_files.clone(), bytecode.source_refs.clone()),
            links: HashMap::default(),
            inverse_links: HashMap::default(),
            memory_indirections: HashMap::default(),
        }
    }

    /// Returns the resultant protocol address of transforming a bytecode operation. Depending on how
    /// the result of the protocol will be consumed, the returned address could refer to an adapter
    /// if the value is consumed like a different type.
    pub fn protocol_address(&self, bytecode_address: BytecodeAddress, ty: &NadaType) -> Option<ProtocolAddress> {
        let bytecode_address = self.resolve_pointer(bytecode_address);
        self.links.get(&bytecode_address).and_then(|accessors| accessors.get(ty)).copied()
    }

    /// Add a protocol to the model
    /// Assigns a protocol memory address to the protocol, and stores it in the model. Previously it resolves the
    /// bytecode address if it is a pointer.
    pub(crate) fn add_protocol(
        &mut self,
        bytecode_address: BytecodeAddress,
        mut protocol: P,
    ) -> Result<ProtocolAddress, Bytecode2ProtocolError> {
        // If the operation is a pointer, we resolve it.
        let bytecode_address = self.resolve_pointer(bytecode_address);
        let ty = protocol.ty();
        // Get the address for this protocol.
        let address = self.address_provider.next_available_address(ty)?;
        // Create a relationship from the bytecode address to the protocol address
        self.create_link_to_protocol(bytecode_address, address, ty)?;
        // Assign the address to the protocol
        protocol.with_address(address);
        // Visit the incoming references to update the information about the protocol relationships
        self.traverse_incoming_references(&protocol)?;
        // Add the protocol to the model.
        self.program.protocols.insert(address, protocol);
        Ok(address)
    }

    fn bytecode_address(&self, address: ProtocolAddress) -> BytecodeAddress {
        match address.1 {
            AddressType::Input | AddressType::Output | AddressType::Literals => address.into(),
            AddressType::Heap => self.inverse_links.get(&address).copied().unwrap_or(address.into()),
        }
    }

    /// Traverse the incoming references and update the protocols relationship
    pub(crate) fn traverse_incoming_references(&mut self, protocol: &P) -> Result<(), Bytecode2ProtocolError> {
        let dependencies = protocol.dependencies();
        for address in dependencies {
            // Update the dependency reads
            self.update_reads(address)?;
        }
        Ok(())
    }

    /// Update the reads of the result of a protocol
    pub(crate) fn update_reads(&mut self, address: ProtocolAddress) -> Result<(), Bytecode2ProtocolError> {
        let bytecode_address = self.bytecode_address(address);
        let ty = self.bytecode.memory_element_type(bytecode_address)?;
        let mut next_address = address;
        for _ in 0..address_count(ty)? {
            let current_references = self.program.reads_table.entry(next_address).or_default();
            *current_references = current_references.wrapping_add(1);
            next_address = next_address.next()?;
        }
        Ok(())
    }

    /// Create a link from the bytecode address to the protocol address.
    pub(crate) fn create_link_to_protocol(
        &mut self,
        bytecode_address: BytecodeAddress,
        address: ProtocolAddress,
        ty: &NadaType,
    ) -> Result<(), Bytecode2ProtocolError> {
        let bytecode_address = self.resolve_pointer(bytecode_address);
        self.inverse_links.insert(address, bytecode_address);
        // Get the protocols that have been created for a bytecode address. We will have different
        // protocol depending on if it has been adapted into another type.
        let transformed_protocols = self.links.entry(bytecode_address).or_default();
        if transformed_protocols.contains_key(ty) {
            return Err(Bytecode2ProtocolError::DuplicateTransformation);
        }
        if ty.is_public() && !ty.is_ecdsa_digest_message() {
            transformed_protocols.insert(ty.as_shamir_share()?, address);
        }
        transformed_protocols.insert(ty.clone(), address);
        Ok(())
    }

    /// Walks across the bytecode indirections and returns the final bytecode address.
    /// In this way, we avoid having memory indirections in the protocol model
    pub(crate) fn resolve_pointer(&self, mut bytecode_address: BytecodeAddress) -> BytecodeAddress {
        // Walk across the indirections
        while let Some(&pointed_address) = self.memory_indirections.get(&bytecode_address) {
            bytecode_address = pointed_address;
        }
        bytecode_address
    }

    /// Crates a pointer from a bytecode address to another bytecode address.
    pub(crate) fn create_pointer(&mut self, bytecode_address: BytecodeAddress, pointed_address: BytecodeAddress) {
        let pointed_address = self.resolve_pointer(pointed_address);
        self.memory_indirections.insert(bytecode_address, pointed_address);
    }

    /// Checks if a bytecode operation has been translated into protocol with a specific type.
    pub fn exist_protocol_of_type(&self, bytecode_address: BytecodeAddress, ty: &NadaType) -> bool {
        self.protocol_address(bytecode_address, ty).is_some()
    }
}

/// Provides the factories to create the protocols from the supported bytecode operations
pub trait ProtocolFactory<P: Protocol>: Copy {
    /// Creates the protocols for a Not operation
    fn create_not(self, context: &mut Bytecode2ProtocolContext<P, Self>, o: &Not) -> Result<P, Bytecode2ProtocolError>;

    /// Creates the protocols for an Addition operation
    fn create_addition(
        self,
        context: &mut Bytecode2ProtocolContext<P, Self>,
        o: &Addition,
    ) -> Result<P, Bytecode2ProtocolError>;

    /// Creates the protocols for a Subtraction operation
    fn create_subtraction(
        self,
        context: &mut Bytecode2ProtocolContext<P, Self>,
        o: &Subtraction,
    ) -> Result<P, Bytecode2ProtocolError>;

    /// Creates the protocols for a Multiplication operation
    fn create_multiplication(
        self,
        context: &mut Bytecode2ProtocolContext<P, Self>,
        o: &Multiplication,
    ) -> Result<P, Bytecode2ProtocolError>;

    /// Creates an array
    fn create_new_array(
        self,
        context: &mut Bytecode2ProtocolContext<P, Self>,
        bytecode_address: BytecodeAddress,
        inner_type: Box<NadaType>,
        size: usize,
    ) -> Result<P, Bytecode2ProtocolError>;

    /// Creates a tuple
    fn create_new_tuple(
        self,
        context: &mut Bytecode2ProtocolContext<P, Self>,
        bytecode_address: BytecodeAddress,
        left_type: Box<NadaType>,
        right_type: Box<NadaType>,
    ) -> Result<P, Bytecode2ProtocolError>;

    /// Creates the protocols for a Modulo operation
    fn create_modulo(
        self,
        context: &mut Bytecode2ProtocolContext<P, Self>,
        o: &Modulo,
    ) -> Result<P, Bytecode2ProtocolError>;

    /// Creates the protocols for a Power operation
    fn create_power(
        self,
        context: &mut Bytecode2ProtocolContext<P, Self>,
        o: &Power,
    ) -> Result<P, Bytecode2ProtocolError>;

    /// Creates the protocols for a Left Shift operation
    fn create_left_shift(
        self,
        context: &mut Bytecode2ProtocolContext<P, Self>,
        o: &LeftShift,
    ) -> Result<P, Bytecode2ProtocolError>;

    /// Creates the protocols for a Right Shift operation
    fn create_right_shift(
        self,
        context: &mut Bytecode2ProtocolContext<P, Self>,
        o: &RightShift,
    ) -> Result<P, Bytecode2ProtocolError>;

    /// Creates the protocols for a Division operation
    fn create_division(
        self,
        context: &mut Bytecode2ProtocolContext<P, Self>,
        o: &Division,
    ) -> Result<P, Bytecode2ProtocolError>;

    /// Creates the protocols for a Less Than operation
    fn create_less_than(
        self,
        context: &mut Bytecode2ProtocolContext<P, Self>,
        o: &LessThan,
    ) -> Result<P, Bytecode2ProtocolError>;

    /// Creates the protocols for an Equals operation
    fn create_equals(
        self,
        context: &mut Bytecode2ProtocolContext<P, Self>,
        o: &Equals,
    ) -> Result<P, Bytecode2ProtocolError>;

    /// Creates the protocols for a Public Output Equality operation
    fn create_public_output_equality(
        self,
        context: &mut Bytecode2ProtocolContext<P, Self>,
        o: &PublicOutputEquality,
    ) -> Result<P, Bytecode2ProtocolError>;

    /// Creates the protocols for an If Else operation
    fn create_if_else(
        self,
        context: &mut Bytecode2ProtocolContext<P, Self>,
        o: &IfElse,
    ) -> Result<P, Bytecode2ProtocolError>;

    /// Creates the protocols for a Reveal operation
    fn create_reveal(
        self,
        context: &mut Bytecode2ProtocolContext<P, Self>,
        o: &Reveal,
    ) -> Result<P, Bytecode2ProtocolError>;

    /// Creates the protocols for a Public Key Derive operation
    fn create_public_key_derive(
        self,
        context: &mut Bytecode2ProtocolContext<P, Self>,
        o: &PublicKeyDerive,
    ) -> Result<P, Bytecode2ProtocolError>;

    /// Creates the protocols for a TruncPr operation
    fn create_trunc_pr(
        self,
        context: &mut Bytecode2ProtocolContext<P, Self>,
        o: &TruncPr,
    ) -> Result<P, Bytecode2ProtocolError>;

    /// Creates the protocols for a EcdsaSign operation
    fn create_ecdsa_sign(
        self,
        context: &mut Bytecode2ProtocolContext<P, Self>,
        o: &EcdsaSign,
    ) -> Result<P, Bytecode2ProtocolError>;

    /// Creates the protocols for a EddsaSign operation
    fn create_eddsa_sign(
        self,
        context: &mut Bytecode2ProtocolContext<P, Self>,
        o: &EddsaSign,
    ) -> Result<P, Bytecode2ProtocolError>;

    /// Creates the protocols for a Random operation
    fn create_random(
        self,
        context: &mut Bytecode2ProtocolContext<P, Self>,
        o: &Random,
    ) -> Result<P, Bytecode2ProtocolError>;

    /// Creates the protocols for a Inner Product operation
    fn create_inner_product(
        self,
        context: &mut Bytecode2ProtocolContext<P, Self>,
        o: &InnerProduct,
    ) -> Result<P, Bytecode2ProtocolError>;

    /// Creates the protocols for a Cast operation
    fn create_cast(
        self,
        context: &mut Bytecode2ProtocolContext<P, Self>,
        o: &Cast,
    ) -> Result<P, Bytecode2ProtocolError>;
}

/// Bytecode to Protocols transformation
pub struct Bytecode2Protocol;

impl Bytecode2Protocol {
    /// Transform a bytecode model into a protocols model.
    pub fn transform<P, F>(
        protocol_factory: F,
        bytecode: &ProgramBytecode,
    ) -> Result<ProtocolsModel<P>, Bytecode2ProtocolError>
    where
        P: Protocol,
        F: ProtocolFactory<P>,
    {
        let mut context = Bytecode2ProtocolContext::new(protocol_factory, bytecode);
        // transforms the literals
        context.program.literals = bytecode.literals().cloned().collect();
        // transforms input scheme
        Self::create_input_memory_scheme(&mut context)?;
        // transforms operations
        for operation in context.bytecode.operations() {
            Self::transform_operation(&mut context, operation)?;
        }
        // transforms output scheme
        for output in context.bytecode.outputs() {
            // We need to traverse the inner values and ensure that the secret values are shares.
            // The public values are public, and they do not have to be updated.
            let type_metadata: NadaTypeMetadata = output.ty().into();
            // If the output is of EcdsaSignature type there is no need to change the shape to ShamirShare type.
            let output_type_metadata = type_metadata.with_shape_if(Shape::ShamirShare, |t| {
                t.is_private().is_some_and(|t| t)
                    && !matches!(t.nada_primitive_type(), Some(NadaPrimitiveType::EcdsaSignature))
            });
            let output_ty: NadaType = (&output_type_metadata).try_into()?;
            let address = Self::adapted_protocol(&mut context, output.inner, &output_ty)?;
            context.update_reads(address)?;
            context
                .program
                .output_memory_scheme
                .insert(output.name.clone(), OutputMemoryAllocation { address, ty: output_ty });
        }
        Ok(context.program)
    }

    /// Creates a new input memory scheme.
    fn create_input_memory_scheme<P, F>(
        context: &mut Bytecode2ProtocolContext<P, F>,
    ) -> Result<(), Bytecode2ProtocolError>
    where
        P: Protocol,
        F: ProtocolFactory<P>,
    {
        let input_memory_scheme = &mut context.program.input_memory_scheme;
        for input in context.bytecode.inputs() {
            let input_name = input.name().to_string();
            let input_type = input.ty();
            context.address_provider.next_available_address(input_type)?;
            input_memory_scheme.insert(
                input.address.into(),
                InputMemoryAllocation { input: input_name, sizeof: address_count(input_type)? },
            );
        }
        Ok(())
    }

    /// Transforms a bytecode operation into a protocol
    fn transform_operation<P, F>(
        context: &mut Bytecode2ProtocolContext<P, F>,
        operation: &Operation,
    ) -> Result<(), Bytecode2ProtocolError>
    where
        P: Protocol,
        F: ProtocolFactory<P>,
    {
        let factory = context.protocol_factory;
        let protocol: P = match operation {
            Operation::Not(o) => factory.create_not(context, o)?,
            Operation::Addition(o) => factory.create_addition(context, o)?,
            Operation::Subtraction(o) => factory.create_subtraction(context, o)?,
            Operation::Multiplication(o) => factory.create_multiplication(context, o)?,
            Operation::Load(o) => return Self::load_input(context, o),
            Operation::Get(o) => {
                context.create_pointer(o.address, o.source_address);
                return Ok(());
            }
            Operation::New(_) => return Ok(()),
            Operation::Modulo(o) => factory.create_modulo(context, o)?,
            Operation::Power(o) => factory.create_power(context, o)?,
            Operation::LeftShift(o) => factory.create_left_shift(context, o)?,
            Operation::RightShift(o) => factory.create_right_shift(context, o)?,
            Operation::Division(o) => factory.create_division(context, o)?,
            Operation::LessThan(o) => factory.create_less_than(context, o)?,
            Operation::Equals(o) => factory.create_equals(context, o)?,
            Operation::PublicOutputEquality(o) => factory.create_public_output_equality(context, o)?,
            Operation::Literal(o) => return Self::load_literal(context, o),
            Operation::IfElse(o) => factory.create_if_else(context, o)?,
            Operation::Reveal(o) => factory.create_reveal(context, o)?,
            Operation::PublicKeyDerive(o) => factory.create_public_key_derive(context, o)?,
            Operation::TruncPr(o) => factory.create_trunc_pr(context, o)?,
            Operation::EcdsaSign(o) => factory.create_ecdsa_sign(context, o)?,
            Operation::EddsaSign(o) => factory.create_eddsa_sign(context, o)?,
            Operation::Random(o) => factory.create_random(context, o)?,
            Operation::InnerProduct(o) => factory.create_inner_product(context, o)?,
            Operation::Cast(o) => factory.create_cast(context, o)?,
        };
        // Adds the protocol to the model that is contained into the context.
        context.add_protocol(operation.address(), protocol)?;
        Ok(())
    }

    /// Returns the address to the protocol that has been transformed from the bytecode memory address
    /// or apply an adapter if it is necessary.
    pub fn adapted_protocol<P, F>(
        context: &mut Bytecode2ProtocolContext<P, F>,
        bytecode_address: BytecodeAddress,
        output_type: &NadaType,
    ) -> Result<ProtocolAddress, Bytecode2ProtocolError>
    where
        P: Protocol,
        F: ProtocolFactory<P>,
    {
        // We have to check if the protocol that we need has been created previously
        if let Some(address) = context.protocol_address(bytecode_address, output_type) {
            return Ok(address);
        }
        // If it does not exist, we have to adapt the protocol
        let bytecode_address = context.resolve_pointer(bytecode_address);
        let factory = context.protocol_factory;

        use nada_type::NadaType::*;
        let protocol = match output_type {
            // If the value is a container, we have to create a new that refers to the inner elements.
            Array { size, inner_type } => {
                factory.create_new_array(context, bytecode_address, inner_type.clone(), *size)?
            }
            Tuple { left_type, right_type } => {
                factory.create_new_tuple(context, bytecode_address, left_type.clone(), right_type.clone())?
            }
            _ => return Err(Bytecode2ProtocolError::AdapterNotFound),
        };

        // Adds the adapted protocol to the model.
        context.add_protocol(bytecode_address, protocol)
    }

    /// Transform a load operation. It is a memory access and for this reason it is not translated
    /// into an any specific protocol.
    pub(crate) fn load_input<P, F>(
        context: &mut Bytecode2ProtocolContext<P, F>,
        load: &Load,
    ) -> Result<(), Bytecode2ProtocolError>
    where
        P: Protocol,
        F: ProtocolFactory<P>,
    {
        let input_ty = context.bytecode.memory_element_type(load.input_address)?;
        let input_address: ProtocolAddress = load.input_address.into();
        let input_ty = input_ty.to_internal_type();
        context.create_link_to_protocol(load.address, input_address, &input_ty)?;
        Ok(())
    }

    /// Transform a literal access operation into an access memory. We don't need any specific
    /// protocol for this.
    fn load_literal<P, F>(
        context: &mut Bytecode2ProtocolContext<P, F>,
        literal_ref: &LiteralRef,
    ) -> Result<(), Bytecode2ProtocolError>
    where
        P: Protocol,
        F: ProtocolFactory<P>,
    {
        let literal_ty = context.bytecode.memory_element_type(literal_ref.literal_id)?;
        let literal_address: ProtocolAddress = literal_ref.literal_id.into();
        // A literal is always public, for this reason, we do not need to adapt into a share.
        context.create_link_to_protocol(literal_ref.address, literal_address, literal_ty)?;
        Ok(())
    }
}
