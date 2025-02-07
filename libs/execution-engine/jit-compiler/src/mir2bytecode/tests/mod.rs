mod mir2bytecode;
mod plan;

use crate::models::{
    bytecode::{memory::BytecodeAddress, Operation, ProgramBytecode},
    memory::AddressType,
};
use anyhow::{bail, Error};
use nada_type::NadaType;

#[allow(clippy::indexing_slicing)]
/// Checks the parties of a program are exactly we expect.
fn assert_parties(bytecode: &ProgramBytecode, parties: Vec<&'static str>) {
    // Check the number of parties are the expected.
    assert_eq!(bytecode.parties.len(), parties.len());

    for party in parties.iter() {
        assert!(parties.contains(party));
    }
}

/// Checks the inputs of a program.
fn assert_inputs(bytecode: &ProgramBytecode, inputs: Vec<(usize, &'static str, NadaType)>) -> Result<(), Error> {
    assert_eq!(bytecode.inputs_count(), inputs.len());
    for (index, name, ty) in inputs.into_iter() {
        let address = BytecodeAddress(index, AddressType::Input);
        let Some(input) = bytecode.input(address)? else {
            panic!("expecting input at address {}", address);
        };
        assert_eq!(input.name, name);
        assert_eq!(&input.ty, &ty);
    }
    Ok(())
}

/// Checks the outputs of a program.
fn assert_outputs(bytecode: &ProgramBytecode, outputs: Vec<(usize, &'static str, NadaType, usize)>) {
    assert_eq!(bytecode.outputs_count(), outputs.len());
    for (output, (index, name, ty, inner)) in bytecode.outputs().zip(outputs.into_iter()) {
        let address = BytecodeAddress(index, AddressType::Output);
        let inner = BytecodeAddress(inner, AddressType::Heap);
        assert_eq!(output.address, address);
        assert_eq!(output.inner, inner);
        assert_eq!(output.name, name);
        assert_eq!(&output.ty, &ty);
    }
}

/// Check if a literal ref matches the expected one.
fn assert_literal_ref(
    bytecode: &ProgramBytecode,
    address: usize,
    ty: NadaType,
    literal_id: usize,
) -> Result<(), Error> {
    let address = BytecodeAddress(address, AddressType::Heap);
    let literal_address = BytecodeAddress(literal_id, AddressType::Literals);
    let operation = bytecode.operation(address)?.unwrap();
    let Operation::Literal(load) = operation else { bail!("expected Literal, found {operation:?}") };
    assert_eq!(load.address, address);
    assert_eq!(load.literal_id, literal_address);
    assert_eq!(&load.ty, &ty);
    Ok(())
}

/// Check if a load operation matches the expected one.
fn assert_load(bytecode: &ProgramBytecode, address: usize, ty: NadaType, input_address: usize) -> Result<(), Error> {
    let address = BytecodeAddress(address, AddressType::Heap);
    let input_address = BytecodeAddress(input_address, AddressType::Input);
    let operation = bytecode.operation(address)?.unwrap();
    let Operation::Load(load) = operation else { bail!("expected load, found {operation:?}") };
    assert_eq!(load.address, address);
    assert_eq!(load.input_address, input_address);
    assert_eq!(&load.ty, &ty);
    Ok(())
}

/// Check if a new operation matches the expected one.
fn assert_new(bytecode: &ProgramBytecode, address: usize, ty: NadaType) -> Result<(), Error> {
    let address = BytecodeAddress(address, AddressType::Heap);
    let operation = bytecode.operation(address)?.unwrap();
    let Operation::New(load) = operation else { bail!("expected new, found {operation:?}") };
    assert_eq!(load.address, address);
    assert_eq!(&load.ty, &ty);
    Ok(())
}

/// Check if a get operation matches the expected one.
fn assert_get(bytecode: &ProgramBytecode, address: usize, ty: NadaType, source: usize) -> Result<(), Error> {
    let address = BytecodeAddress(address, AddressType::Heap);
    let source_address = BytecodeAddress(source, AddressType::Heap);
    let operation = bytecode.operation(address)?.unwrap();
    let Operation::Get(operation) = operation else { bail!("expected Get, found {operation:?}") };
    assert_eq!(operation.address, address);
    assert_eq!(operation.source_address, source_address);
    assert_eq!(&operation.ty, &ty);
    Ok(())
}

/// Check if a not operation matches the expected one.
fn assert_not(bytecode: &ProgramBytecode, address: usize, ty: NadaType, operand: usize) -> Result<(), Error> {
    let address = BytecodeAddress(address, AddressType::Heap);
    let operand = BytecodeAddress(operand, AddressType::Heap);
    let operation = bytecode.operation(address)?.unwrap();
    let Operation::Not(operation) = operation else { bail!("expected Not, found {operation:?}") };
    assert_eq!(operation.address, address);
    assert_eq!(operation.operand, operand);
    assert_eq!(&operation.ty, &ty);
    Ok(())
}

/// Check if a reveal operation matches the expected one.
fn assert_reveal(bytecode: &ProgramBytecode, address: usize, ty: NadaType, operand: usize) -> Result<(), Error> {
    let address = BytecodeAddress(address, AddressType::Heap);
    let operand = BytecodeAddress(operand, AddressType::Heap);
    let operation = bytecode.operation(address)?.unwrap();
    let Operation::Reveal(operation) = operation else { bail!("expected Reveal, found {operation:?}") };
    assert_eq!(operation.address, address);
    assert_eq!(operation.operand, operand);
    assert_eq!(&operation.ty, &ty);
    Ok(())
}

/// Generate assert for a binary operation
macro_rules! assert_binary_operation {
    ($o:ident, $fn_name:ident) => {
        #[doc = concat!("Check if a ", stringify!($o)," operation matches the expected one. ")]
        fn $fn_name(
            bytecode: &ProgramBytecode,
            address: usize,
            ty: NadaType,
            left: usize,
            right: usize,
        ) -> Result<(), Error> {
            let address = BytecodeAddress(address, AddressType::Heap);
            let left_address = BytecodeAddress(left, AddressType::Heap);
            let right_address = BytecodeAddress(right, AddressType::Heap);
            let operation = bytecode.operation(address)?.unwrap();
            let Operation::$o(operation) = operation else { bail!("expected {}, found {operation:?}", stringify!($o)) };
            assert_eq!(
                operation.address, address,
                "operation address expected: {}, actual: {}",
                operation.address, address
            );
            assert_eq!(
                operation.left, left_address,
                "binary operation left address expected {} actual {}",
                operation.left, left_address
            );
            assert_eq!(
                operation.right, right_address,
                "binary operation right address expected {} actual {}",
                operation.right, right_address
            );
            assert_eq!(&operation.ty, &ty);
            Ok(())
        }
    };
}

assert_binary_operation!(Addition, assert_addition);
assert_binary_operation!(Subtraction, assert_subtraction);
assert_binary_operation!(Multiplication, assert_multiplication);
assert_binary_operation!(Modulo, assert_modulo);
assert_binary_operation!(Power, assert_power);
assert_binary_operation!(LeftShift, assert_left_shift);
assert_binary_operation!(RightShift, assert_right_shift);
assert_binary_operation!(Division, assert_division);
assert_binary_operation!(LessThan, assert_less_than);
assert_binary_operation!(PublicOutputEquality, assert_public_output_equality);
assert_binary_operation!(EcdsaSign, assert_ecdsa_sign);

fn assert_if_else(
    bytecode: &ProgramBytecode,
    address: usize,
    ty: NadaType,
    first: usize,
    second: usize,
    third: usize,
) -> Result<(), Error> {
    let address = BytecodeAddress(address, AddressType::Heap);
    let first = BytecodeAddress(first, AddressType::Heap);
    let second = BytecodeAddress(second, AddressType::Heap);
    let third = BytecodeAddress(third, AddressType::Heap);
    let operation = bytecode.operation(address)?.unwrap();
    let Operation::IfElse(operation) = operation else { bail!("expected Reveal, found {operation:?}") };
    assert_eq!(operation.address, address);
    assert_eq!(operation.first, first);
    assert_eq!(operation.second, second);
    assert_eq!(operation.third, third);
    assert_eq!(&operation.ty, &ty);
    Ok(())
}
