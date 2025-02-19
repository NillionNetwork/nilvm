use crate::operations::{
    AddOperation, BinaryOperation, DivOperation, EqualsOperation, IfElseOperation, LeftShiftOperation, LtOperation,
    ModuloOperation, MulOperation, NotOperation, OperationDisplay, PowerOperation, PublicOutputEqualityOperation,
    RevealOperation, RightShiftOperation, SubOperation, TernaryOperation, TruncPrOperation, UnaryOperation,
};
use anyhow::{anyhow, Error};
use jit_compiler::models::{
    bytecode::{
        memory::BytecodeAddress, Addition, Division, EcdsaSign, Equals, Get, IfElse, InnerProduct, Input, LeftShift,
        LessThan, LiteralRef, Load, Modulo, Multiplication, New, Not, Operation, Power, ProgramBytecode,
        PublicOutputEquality, Random, Reveal, RightShift, Subtraction, TruncPr,
    },
    memory::{address_count, AddressType},
};
use log::{debug, info};
use math_lib::{
    conversions::boolean_from_bigint,
    errors::DivByZero,
    impl_boxed_from_encoded_safe_prime,
    modular::{Modular, ModularNumber, Overflow, Prime, SafePrime},
};
use nada_compiler_backend::{
    literal_value::LiteralValue,
    mir::{NamedElement, TypedElement},
};
use nada_value::{
    clear::Clear,
    clear_modular::ClearModular,
    errors::{ClearModularError, NonPrimitiveValue},
    NadaType, NadaTypeMetadata, NadaValue, Shape, TypeError,
};
use num_bigint::{BigInt, BigUint};
use operations::InnerProductOperation;
use std::{collections::HashMap, marker::PhantomData, vec};

pub(crate) mod operations;

#[cfg(test)]
mod tests;

/// An error during the construction or evaluation.
#[derive(Debug, thiserror::Error)]
pub enum EvaluationError {
    /// The left and right operands have a type mismatch.
    #[error("type mismatch")]
    MismatchedTypes,

    /// Non primitive value.
    #[error(transparent)]
    NonPrimitiveValue(#[from] NonPrimitiveValue),

    /// This error happens when a conversion from clear value to modular values fails.
    #[error(transparent)]
    ModularType(#[from] ClearModularError),

    /// Type error.
    #[error(transparent)]
    Unimplemented(#[from] TypeError),

    /// Overflow.
    #[error("overflow")]
    Overflow(#[from] Overflow),

    /// Division by Zero
    #[error("division by zero")]
    DivByZero(#[from] DivByZero),

    /// Invalid operand types
    #[error("invalid operand types")]
    InvalidOperandTypes,

    /// Operand is not allowed
    #[error("operand is not allowed: {0}")]
    NotAllowedOperand(&'static str),
}

pub(crate) enum BytecodeMemoryElement<T: SafePrime> {
    /// Header memory element. Stores the type of compound elements
    Header(NadaType),
    /// Value memory element, stores a [`NadaValue`]
    Value(NadaValue<ClearModular<T>>),
}

/// The heap memory
pub struct HeapMemory<T: SafePrime>(Vec<BytecodeMemoryElement<T>>);

impl<T: SafePrime> HeapMemory<T> {
    pub(crate) fn new() -> Self {
        Self(vec![])
    }

    pub(crate) fn get_value(&self, address: BytecodeAddress) -> Result<&NadaValue<ClearModular<T>>, Error> {
        if address.1 != AddressType::Heap {
            return Err(anyhow!("address {address:?} is not in the heap"));
        }
        let element = self.0.get(address.0).ok_or(anyhow!("address {address} not found in the heap"))?;

        let BytecodeMemoryElement::Value(value) = element else {
            return Err(anyhow!("tried to access a non-value memory element"));
        };
        Ok(value)
    }

    pub(crate) fn get_type(&self, address: BytecodeAddress) -> Result<NadaType, Error> {
        if address.1 != AddressType::Heap {
            return Err(anyhow!("address {address:?} is not in the heap"));
        }
        let element = self.0.get(address.0).ok_or(anyhow!("address {address} not found in the heap"))?;
        match element {
            BytecodeMemoryElement::Header(ty) => Ok(ty.clone()),
            BytecodeMemoryElement::Value(value) => Ok(value.to_type()),
        }
    }

    pub(crate) fn push_value(&mut self, value: NadaValue<ClearModular<T>>) -> Result<(), Error> {
        if value.to_type().is_primitive() {
            self.0.push(BytecodeMemoryElement::Value(value));
            Ok(())
        } else {
            Err(anyhow!("cannot push a non primitive value"))
        }
    }

    pub(crate) fn push_header(&mut self, ty: NadaType) -> Result<(), Error> {
        if ty.is_primitive() {
            Err(anyhow!("cannot push a header for a primitive value"))
        } else {
            self.0.push(BytecodeMemoryElement::Header(ty));
            Ok(())
        }
    }

    pub(crate) fn len(&self) -> usize {
        self.0.len()
    }
}

pub struct Evaluator<T: SafePrime> {
    inputs: Vec<NadaValue<ClearModular<T>>>,
    literals: HashMap<String, NadaValue<ClearModular<T>>>,
    heap: HeapMemory<T>,
    outputs: Vec<NadaValue<ClearModular<T>>>,
    _unused: PhantomData<T>,
}

impl<T: SafePrime> Default for Evaluator<T> {
    fn default() -> Self {
        Self {
            inputs: Vec::new(),
            literals: HashMap::new(),
            heap: HeapMemory::new(),
            outputs: Vec::new(),
            _unused: PhantomData,
        }
    }
}

impl<T: SafePrime> Evaluator<T> {
    pub fn run(
        bytecode: &ProgramBytecode,
        inputs: HashMap<String, NadaValue<Clear>>,
    ) -> Result<HashMap<String, NadaValue<Clear>>, Error> {
        info!("{}", bytecode.header_text_repr());

        let mut evaluator: Evaluator<T> = Evaluator::default();
        info!("\nLoading Literals:");
        evaluator.store_literals(bytecode)?;
        info!("\nLoading Inputs:");
        evaluator.store_inputs(bytecode, inputs)?;
        info!("\nComputing:");
        evaluator.simulate(bytecode)?;
        info!("\nLoading Outputs:");

        let result = evaluator.load_outputs(bytecode);
        info!("\n");
        result
    }

    /// Loads all outputs from the program's memory. It's executed when the execution has finished to
    /// return the result.
    fn load_outputs(self, bytecode: &ProgramBytecode) -> Result<HashMap<String, NadaValue<Clear>>, Error> {
        let mut outputs: HashMap<String, NadaValue<Clear>> = HashMap::new();
        let mut outputs_iterator = bytecode.outputs();
        let mut output = if let Some(next_output) = outputs_iterator.next() {
            next_output
        } else {
            return Ok(outputs); // The bytecode doesn't expect outputs
        };
        #[allow(clippy::type_complexity)]
        let mut compound_elements: Vec<(NadaValue<ClearModular<T>>, Vec<NadaValue<Clear>>)> = vec![];
        for element in self.outputs.into_iter() {
            let ty = element.to_type();
            if ty.is_array() || ty.is_tuple() {
                // If the element is a compound type, then we have to add to into compound_elements,
                // because we have built its inner_elements.
                compound_elements.push((element, vec![]));
                continue;
            }

            // From here, the element doesn't contain any element and it is completed.
            let mut element = Some(memory_element_into_output(&element, vec![])?);
            // This while checks if an element is an output or an inner element.
            while let Some(inner_element) = element {
                if let Some((compound_element, mut inner_elements)) = compound_elements.pop() {
                    // We are building at less a compound element and, we have an inner_element.
                    // It is pushed into the list of inner element of the compound element that is
                    // in the top of compound_elements.
                    inner_elements.push(inner_element);

                    match output.ty() {
                        NadaType::Array { size, .. } if *size == inner_elements.len() => {
                            // If compound_element is completed, we have to iterate and check if it is an output
                            // or it's an inner_element.
                            element = Some(memory_element_into_output(&compound_element, inner_elements)?);
                        }
                        NadaType::Tuple { .. } if inner_elements.len() == 2 => {
                            // If compound_element is completed, we have to iterate and check if it is an output
                            // or it's an inner_element.
                            element = Some(memory_element_into_output(&compound_element, inner_elements)?);
                        }
                        _ => {
                            // If the compound_element isn't completed, we'll continue getting elements
                            // from the output memory.
                            compound_elements.push((compound_element, inner_elements));
                            element = None;
                        }
                    }
                } else {
                    let output_text_repr = output.text_repr(bytecode);
                    info!("{output_text_repr}\n  {inner_element:?}");
                    outputs.insert(output.name.clone(), inner_element);

                    output = if let Some(next_output) = outputs_iterator.next() {
                        next_output
                    } else {
                        return Ok(outputs); // We have retrieved all outputs
                    };

                    element = None;
                }
            }
        }
        Ok(outputs)
    }

    fn store_literals(&mut self, bytecode: &ProgramBytecode) -> Result<(), Error> {
        for literal in bytecode.literals() {
            let memory_element = memory_element_from_literal(&literal.value)?;
            info!("{literal}\n  {memory_element:?}");
            self.literals.insert(literal.name.clone(), memory_element);
        }
        Ok(())
    }

    fn store_inputs(
        &mut self,
        bytecode: &ProgramBytecode,
        mut inputs: HashMap<String, NadaValue<Clear>>,
    ) -> Result<(), Error> {
        // We have to locate the inputs and load them into the program's input memory.
        for bytecode_input in bytecode.inputs() {
            let input_name = bytecode_input.name();
            // Read inputs
            let input = inputs.remove(input_name).ok_or(anyhow!("program requires an input {input_name} not found"))?;
            Self::input_typecheck(bytecode_input, &input.to_type())?;
            let input: NadaValue<ClearModular<T>> = input.try_into()?;
            self.inputs.extend(input.flatten_inner_values());
        }

        Ok(())
    }

    /// Checks whether the type of program input matches the input type provided
    ///
    /// # Arguments
    /// * `bytecode_input` - The input found in the program bytecode
    /// * `provided_input_type` - The input type corresponding to the provided input
    fn input_typecheck(bytecode_input: &Input, provided_input_type: &NadaType) -> Result<(), Error> {
        let bytecode_input_type = &bytecode_input.ty;

        if provided_input_type != bytecode_input_type {
            return Err(anyhow!(
                "type mismatch for input \"{}\": was {provided_input_type}, expected {bytecode_input_type}",
                bytecode_input.name
            ));
        }
        Ok(())
    }

    /// Reads the value in a memory position. It only works if the value is primitive
    fn allocated_element_value(&self, address: BytecodeAddress) -> Result<&NadaValue<ClearModular<T>>, Error> {
        let allocated_element = match address.1 {
            AddressType::Input => self.inputs.get(address.0),
            AddressType::Output => self.outputs.get(address.0),
            AddressType::Heap => Some(self.heap.get_value(address)?),
            AddressType::Literals => Err(anyhow!("support for literals memory address is not implemented"))?,
        };
        allocated_element.ok_or_else(|| anyhow!("error memory access: {address:?}"))
    }

    /// Reads an element from memory.
    ///
    /// If the element is primitive it returns the corresponding value.
    /// If the element is compound it reads all the inner elements of the compound type
    /// and returns them together with the parent element.
    pub(crate) fn read_memory_element(&self, address: BytecodeAddress) -> Result<NadaValue<ClearModular<T>>, Error> {
        let ty = self.heap.get_type(address)?;
        if ty.is_primitive() {
            Ok(self.allocated_element_value(address)?.clone())
        } else {
            // If the element is a compound type, we add it to the list of compound elements
            // This list will help us track and build all the compound elements iteratively
            // Since we store new arrays with empty elements we need to void the inner elements
            use NadaType::*;
            match ty {
                Array { inner_type, size } => {
                    let mut values = vec![];
                    for i in 1..=size {
                        let inner_element_address = address.advance(i)?;
                        values.push(self.read_memory_element(inner_element_address)?);
                    }
                    Ok(NadaValue::new_array(*inner_type, values)?)
                }
                Tuple { .. } => Ok(NadaValue::new_tuple(
                    self.read_memory_element(address.advance(1)?)?,
                    self.read_memory_element(address.advance(2)?)?,
                )?),
                NTuple { types } => {
                    let mut values = vec![];
                    for i in 1..=types.len() {
                        let inner_element_address = address.advance(i)?;
                        values.push(self.read_memory_element(inner_element_address)?);
                    }
                    Ok(NadaValue::new_n_tuple(values)?)
                }
                Object { types } => {
                    let mut values = vec![];
                    for i in 1..=types.len() {
                        let inner_element_address = address.advance(i)?;
                        values.push(self.read_memory_element(inner_element_address)?);
                    }
                    Ok(NadaValue::new_object(types.keys().cloned().zip(values.into_iter()).collect())?)
                }
                Integer
                | UnsignedInteger
                | Boolean
                | SecretInteger
                | SecretUnsignedInteger
                | SecretBoolean
                | ShamirShareInteger
                | ShamirShareUnsignedInteger
                | ShamirShareBoolean
                | SecretBlob
                | EcdsaDigestMessage
                | EcdsaPrivateKey
                | EcdsaSignature
                | EcdsaPublicKey
                | StoreId
                | EddsaPrivateKey
                | EddsaPublicKey
                | EddsaSignature
                | EddsaMessage => Err(anyhow!("type is not compound")),
            }
        }
    }

    fn simulate(&mut self, bytecode: &ProgramBytecode) -> Result<(), Error> {
        for operation in bytecode.operations() {
            let operation_text_repr = operation.text_repr(bytecode);

            match operation {
                Operation::Addition(Addition { left, right, .. }) => {
                    self.run_binary_operation(*left, *right, operation_text_repr, AddOperation)?;
                }
                Operation::Subtraction(Subtraction { left, right, .. }) => {
                    self.run_binary_operation(*left, *right, operation_text_repr, SubOperation)?;
                }
                Operation::Multiplication(Multiplication { left, right, .. }) => {
                    self.run_binary_operation(*left, *right, operation_text_repr, MulOperation)?;
                }
                Operation::Modulo(Modulo { left, right, .. }) => {
                    self.run_binary_operation(*left, *right, operation_text_repr, ModuloOperation)?;
                }
                Operation::Power(Power { left, right, .. }) => {
                    self.run_binary_operation(*left, *right, operation_text_repr, PowerOperation)?;
                }
                Operation::Division(Division { left, right, .. }) => {
                    self.run_binary_operation(*left, *right, operation_text_repr, DivOperation)?;
                }
                Operation::LessThan(LessThan { left, right, .. }) => {
                    self.run_binary_operation(*left, *right, operation_text_repr, LtOperation)?;
                }
                Operation::LeftShift(LeftShift { left, right, .. }) => {
                    self.run_binary_operation(*left, *right, operation_text_repr, LeftShiftOperation)?;
                }
                Operation::RightShift(RightShift { left, right, .. }) => {
                    self.run_binary_operation(*left, *right, operation_text_repr, RightShiftOperation)?;
                }
                Operation::TruncPr(TruncPr { left, right, .. }) => {
                    self.run_binary_operation(*left, *right, operation_text_repr, TruncPrOperation)?;
                }
                Operation::PublicOutputEquality(PublicOutputEquality { left, right, .. }) => {
                    self.run_binary_operation(*left, *right, operation_text_repr, PublicOutputEqualityOperation)?;
                }
                Operation::Equals(Equals { left, right, .. }) => {
                    self.run_binary_operation(*left, *right, operation_text_repr, EqualsOperation)?;
                }
                Operation::Not(Not { operand, .. }) => {
                    self.run_unary_operation(*operand, operation_text_repr, NotOperation)?;
                }
                Operation::Load(Load { input_address, .. }) => {
                    let allocated_element = self.allocated_element_value(*input_address)?.clone();
                    info!("{operation_text_repr}\n  {allocated_element:?}");
                    self.heap.push_value(allocated_element)?;
                }
                Operation::Get(Get { source_address, .. }) => {
                    let ty = self.heap.get_type(*source_address)?;
                    if ty.is_primitive() {
                        let allocated_element = self.allocated_element_value(*source_address)?.clone();
                        info!("{operation_text_repr}\n  {allocated_element:?}");
                        self.heap.push_value(allocated_element)?;
                    } else {
                        self.heap.push_header(ty)?;
                    }
                }
                Operation::New(New { ty, .. }) => {
                    info!("{operation_text_repr}\n  {ty:?}");
                    self.heap.push_header(ty.clone())?;
                }
                Operation::Literal(LiteralRef { literal_id, .. }) => {
                    let literal = bytecode.literal(*literal_id)?.ok_or_else(|| anyhow!("literal not found"))?;
                    let name = literal.name.clone();
                    let literal = self.literals.get(&name).ok_or_else(|| {
                        anyhow!(
                            "literal {name} not found, available literals: {:?}",
                            bytecode.literals().collect::<Vec<_>>()
                        )
                    })?;
                    info!("{operation_text_repr}\n  {literal:?}");
                    self.heap.push_value(literal.clone())?;
                }
                Operation::Cast(_) => Err(anyhow!("unsupported operation"))?,
                Operation::IfElse(IfElse { first, second, third, .. }) => {
                    self.run_ternary_operation(*first, *second, *third, operation_text_repr, IfElseOperation)?;
                }
                Operation::Random(Random { ty, address, .. }) => match ty {
                    NadaType::SecretInteger | NadaType::SecretUnsignedInteger => {
                        let value = ModularNumber::gen_random();
                        let result = NadaValue::from_iter(Some(value), ty.clone())?;
                        debug!("[Heap {}] new random [Input {}]", self.heap.len() + 1, address.0);
                        self.heap.push_value(result)?;
                    }
                    NadaType::SecretBoolean => {
                        let value = (ModularNumber::gen_random() % &ModularNumber::two())?;
                        let result = NadaValue::from_iter(Some(value), ty.clone())?;
                        debug!("[Heap {}] new random [Input {}]", self.heap.len() + 1, address.0);
                        self.heap.push_value(result)?;
                    }
                    _ => Err(anyhow!("unsupported type for random operation: {:?}", ty))?,
                },
                Operation::Reveal(Reveal { operand, .. }) => {
                    self.run_unary_operation(*operand, operation_text_repr, RevealOperation)?;
                }
                Operation::InnerProduct(InnerProduct { left, right, .. }) => {
                    self.run_binary_operation(*left, *right, operation_text_repr, InnerProductOperation)?;
                }
                Operation::EcdsaSign(EcdsaSign { .. }) => {
                    return Err(anyhow!("EcdsaSign operation is not implemented by the bytecode-evaluator"));
                }
            }
        }

        // We load the memory elements from the heap to the program's output memory
        for output in bytecode.outputs() {
            let output_result = self.read_memory_element(output.inner)?.clone();
            self.outputs.push(output_result);
            // If the output is an array, we have to move the array content to the output memory
            for memory_offset in 1..address_count(&output.ty)? {
                // We have to ignore the first memory position because is the new that we have just move above.
                // We add the array offset (the address of the 'new' operation) to get the address
                // of the array's element.
                let memory_offset = BytecodeAddress::new(
                    memory_offset + output.inner.0,
                    AddressType::Heap, // The outputs are always load from the heap.
                );
                let allocated_element = self.allocated_element_value(memory_offset)?.clone();
                self.outputs.push(allocated_element);
            }
        }
        Ok(())
    }

    fn run_ternary_operation(
        &mut self,
        first: BytecodeAddress,
        second: BytecodeAddress,
        third: BytecodeAddress,
        operation_text_repr: String,
        operation: impl TernaryOperation,
    ) -> Result<(), Error> {
        let OperationDisplay { symbol, .. } = operation.display_info();
        let first_address = self.allocated_element_value(first)?;
        let second_hs = self.heap.get_value(second)?;
        let third_hs = self.heap.get_value(third)?;
        let operation_type = operation.output_type(first_address, second_hs, third_hs)?;
        let value = operation.execute(first_address.clone(), second_hs.clone(), third_hs.clone())?;
        let result = NadaValue::from_iter(Some(value), operation_type)?;
        info!("{operation_text_repr}\n  {result:?} = {symbol} {first:?} {second:?} {third_hs:?}");
        self.heap.push_value(result)?;
        Ok(())
    }

    fn run_binary_operation(
        &mut self,
        left: BytecodeAddress,
        right: BytecodeAddress,
        operation_text_repr: String,
        operation: impl BinaryOperation,
    ) -> Result<(), Error> {
        let OperationDisplay { symbol, .. } = operation.display_info();
        let lhs = self.read_memory_element(left.as_heap())?;
        let rhs = self.read_memory_element(right.as_heap())?;
        let operation_type = operation.output_type(&lhs, &rhs)?;
        debug!(
            "Operation: {}, left_ty: {:?}, right_ty: {:?}, output_ty: {:?}",
            operation.display_info().name,
            lhs.to_type(),
            rhs.to_type(),
            operation_type
        );
        let value = operation.execute(lhs.clone(), rhs.clone())?;
        let result = NadaValue::from_iter(Some(value), operation_type)?;
        info!("{operation_text_repr}\n  {result:?} = {lhs:?} {symbol} {rhs:?}");
        self.heap.push_value(result)?;
        Ok(())
    }

    fn run_unary_operation(
        &mut self,
        operand_address: BytecodeAddress,
        operation_text_repr: String,
        operation: impl UnaryOperation,
    ) -> Result<(), Error> {
        let symbol = operation.display_info().symbol;
        let operand = self.allocated_element_value(operand_address)?;
        let operation_type = operation.output_type(operand)?;
        let value = operation.execute(operand.clone())?;
        let result = NadaValue::from_iter(Some(value), operation_type)?;
        info!("{operation_text_repr}\n  {result:?} = {operand:?} {symbol}");
        self.heap.push_value(result)?;
        Ok(())
    }
}

pub(crate) fn memory_element_from_literal<T: Modular>(
    value: &LiteralValue,
) -> Result<NadaValue<ClearModular<T>>, Error> {
    match value {
        NadaValue::Integer(value) => Ok(NadaValue::new_integer(ModularNumber::try_from(value)?)),
        NadaValue::UnsignedInteger(value) => Ok(NadaValue::new_unsigned_integer(ModularNumber::try_from(value)?)),
        NadaValue::Boolean(value) => {
            let value = ModularNumber::try_from(&BigUint::from(*value as u32))?;
            Ok(NadaValue::new_boolean(value))
        }
        value if !value.to_type().is_public() => Err(anyhow!("literals cannot be secrets"))?,
        value => Err(anyhow!("{} public variables", value.to_type()))?,
    }
}

fn memory_element_into_output<T: Prime>(
    memory_element: &NadaValue<ClearModular<T>>,
    content: Vec<NadaValue<Clear>>,
) -> Result<NadaValue<Clear>, Error> {
    match memory_element {
        NadaValue::SecretInteger(value) => Ok(NadaValue::new_secret_integer(value)),
        NadaValue::SecretUnsignedInteger(value) => Ok(NadaValue::new_secret_unsigned_integer(value)),
        NadaValue::SecretBoolean(value) => {
            let value = BigInt::from(value);
            Ok(NadaValue::new_secret_boolean(boolean_from_bigint(value)?))
        }
        NadaValue::Integer(value) => Ok(NadaValue::new_integer(value)),
        NadaValue::UnsignedInteger(value) => Ok(NadaValue::new_unsigned_integer(value)),
        NadaValue::Boolean(value) => {
            let value = BigInt::from(value);
            Ok(NadaValue::new_boolean(boolean_from_bigint(value)?))
        }
        NadaValue::Array { inner_type, .. } => {
            let metadata: NadaTypeMetadata = inner_type.into();
            let metadata = metadata.with_shape(Shape::Secret);
            let inner_type: NadaType = (&metadata).try_into()?;
            Ok(NadaValue::new_array(inner_type, content)?)
        }
        NadaValue::Tuple { .. } => {
            if content.len() != 2 {
                return Err(anyhow!("expected two elements in content, got {}", content.len()));
            }
            Ok(NadaValue::new_tuple(content[0].clone(), content[1].clone())?)
        }
        memory_element => Err(anyhow!("type is not supported: {}", memory_element.to_type())),
    }
}

#[derive(Default)]
struct PrimeRunner<T>(PhantomData<T>);

pub trait EvaluatorRunner {
    fn run(
        &self,
        bytecode: &ProgramBytecode,
        values: HashMap<String, NadaValue<Clear>>,
    ) -> Result<HashMap<String, NadaValue<Clear>>, Error>;
}

impl<T: SafePrime> EvaluatorRunner for PrimeRunner<T> {
    fn run(
        &self,
        bytecode: &ProgramBytecode,
        values: HashMap<String, NadaValue<Clear>>,
    ) -> Result<HashMap<String, NadaValue<Clear>>, Error> {
        Evaluator::<T>::run(bytecode, values)
    }
}

impl_boxed_from_encoded_safe_prime!(PrimeRunner, EvaluatorRunner);
