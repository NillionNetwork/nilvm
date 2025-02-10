use std::env::current_dir;

use anyhow::{anyhow, bail, Error};

use nada_compiler_backend::{
    mir::{
        Addition, IfElse, InputReference, LessThan, MIRProgramMalformed, Map, Multiplication, NadaFunction,
        NadaFunctionCall, Operation, OperationId, OperationIdGenerator, ProgramMIR, Reduce, TupleIndex, TypedElement,
    },
    preprocess::{error::MIRPreprocessorError, preprocessor::preprocess},
};
use nada_value::NadaType;
use pynadac::Compiler;

use crate::assert::*;

fn read_test_mir(program: &str) -> Result<ProgramMIR, Error> {
    let cwd = current_dir().expect("failed to get cwd");
    let mut root = "../test-programs/programs".to_string();
    if !cwd.ends_with("compiler-backend-tests") {
        root = format!("nada-lang/compiler-backend-tests/{root}");
    }
    let program_path = format!("{root}/{program}.py");
    Compiler::eval_program(program_path).map(|o| o.mir)
}

#[test]
fn preprocess_le() -> Result<(), Error> {
    let mir = read_test_mir("less_or_equal_than")?;
    let lt_id = mir.outputs[0].operation_id;
    let le_op = get_less_or_equal_than(&mir, lt_id)?;
    let expected_instrunction_count = mir.operations.len() + 1;
    let expected_not_id = le_op.id;
    let expected_lt_left_id = le_op.right;
    let expected_lt_right_id = le_op.left;
    let preprocessed_mir = preprocess(mir)?;
    assert_eq!(preprocessed_mir.operations.len(), expected_instrunction_count);
    let not_op = assert_not(&preprocessed_mir, expected_not_id)?;
    assert_less_than(&preprocessed_mir, not_op.this, expected_lt_left_id, expected_lt_right_id)?;
    Ok(())
}

#[test]
fn preprocess_gt() -> Result<(), Error> {
    let mir = read_test_mir("greater_than")?;
    let gt_id = mir.outputs[0].operation_id;
    let gt_op = get_greater_than(&mir, gt_id)?;
    let expected_instrunction_count = mir.operations.len();
    let expected_lt_id = gt_op.id;
    let expected_lt_left_id = gt_op.right;
    let expected_lt_right_id = gt_op.left;
    let preprocessed_mir = preprocess(mir)?;
    assert_eq!(preprocessed_mir.operations.len(), expected_instrunction_count);
    assert_less_than(&preprocessed_mir, expected_lt_id, expected_lt_left_id, expected_lt_right_id)?;
    Ok(())
}

#[test]
fn preprocess_ge() -> Result<(), Error> {
    let mir = read_test_mir("greater_or_equal_than")?;
    let ge_id = mir.outputs[0].operation_id;
    let ge_op = get_greater_or_equal_than(&mir, ge_id)?;
    let expected_instrunction_count = mir.operations.len() + 1;
    let expected_not_id = ge_op.id;
    let expected_lt_left_id = ge_op.left;
    let expected_lt_right_id = ge_op.right;
    let preprocessed_mir = preprocess(mir)?;
    assert_eq!(preprocessed_mir.operations.len(), expected_instrunction_count);
    let not_op = assert_not(&preprocessed_mir, expected_not_id)?;
    assert_less_than(&preprocessed_mir, not_op.this, expected_lt_left_id, expected_lt_right_id)?;
    Ok(())
}

#[test]
// new_array = my_array_1.zip(my_array_2)
fn preprocess_zip() -> Result<(), Error> {
    let mir = read_test_mir("zip_simple")?;
    let zip_id = mir.outputs[0].operation_id;
    let zip_op = get_zip(&mir, zip_id)?;
    let NadaType::Array { size, .. } = zip_op.ty else {
        bail!("'zip' type is not an array");
    };
    // operations:
    // - 2 IdentifierReferences
    // - New (Array)
    // - for each element in the array
    //   - New (Tuple)
    //   - ArrayAccessor (left brach)
    //   - ArrayAccessor (right branch)
    let expected_instrunction_count = 3 + size * 3;
    let zip_id = zip_op.id;
    let zip_left_id = zip_op.left;
    let zip_right_id = zip_op.right;
    let preprocessed_mir = preprocess(mir)?;
    assert_eq!(preprocessed_mir.operations.len(), expected_instrunction_count);
    // Check that the New Array is well-formed
    let inner_tuple_type =
        NadaType::Tuple { left_type: Box::new(NadaType::SecretInteger), right_type: Box::new(NadaType::SecretInteger) };
    let zip_ty = NadaType::Array { inner_type: Box::new(inner_tuple_type.clone()), size };
    let new_op = assert_new(&preprocessed_mir, zip_id, &zip_ty, size)?;

    // Check that the inner elements of the resultant array are well-formed
    for (index, inner_element_id) in new_op.elements.iter().enumerate() {
        // Check that the New Tuple is well-formed
        let new_inner_tuple = assert_new(&preprocessed_mir, *inner_element_id, &inner_tuple_type, 2)?;
        // Check that the left branch is well-formed
        let left_brach_id = new_inner_tuple.elements[0];
        assert_array_accessor(&preprocessed_mir, left_brach_id, &NadaType::SecretInteger, zip_left_id, index)?;
        // Check that the right branch is well-formed
        let right_branch_id = new_inner_tuple.elements[1];
        assert_array_accessor(&preprocessed_mir, right_branch_id, &NadaType::SecretInteger, zip_right_id, index)?;
    }
    Ok(())
}
/// Assert input reference.
///
/// Utility function that asserts that:
/// - the MIR has an operation with identifier `operation_id`
/// - This operation is an `InputReference`
/// - The `InputReference` refers to the input with name `input_name`
///
/// # Returns
/// A reference to the operation found
fn assert_input_reference<'a>(mir: &'a ProgramMIR, operation_id: OperationId, input_name: &str) -> &'a Operation {
    let op =
        mir.operation(operation_id).map_err(|_| anyhow!("missing input operation for input {input_name}")).unwrap();
    let _input_name = input_name.to_string();
    assert!(matches!(op, Operation::InputReference(InputReference { refers_to: _input_name, .. })));
    &op
}

/// Checks that `nada_fn_simple` is inlined properly.
#[test]
fn test_preprocess_function_simple() -> Result<(), Error> {
    let mir = read_test_mir("nada_fn_simple")?;

    let mir = preprocess(mir)?;
    let addition_id = mir.outputs[0].operation_id;
    let addition_op = mir.operation(addition_id).map_err(|_| anyhow!("missing addition operation"))?;
    if let Operation::Addition(Addition { left: result_left, right: result_right, .. }) = addition_op {
        let left_op = assert_input_reference(&mir, *result_left, "a");
        let right_op = assert_input_reference(&mir, *result_right, "b");
        assert_eq!(*result_left, left_op.id());
        assert_eq!(*result_right, right_op.id());
    } else {
        panic!("Invalid result operation {:?}", addition_op);
    }

    Ok(())
}

/// checks that `nada_fn_add_mul` is preprocessed properly
#[test]
fn test_preprocess_function_with_reuse() -> Result<(), Error> {
    // Defines a function: a * (a + b)
    let mir = read_test_mir("nada_fn_add_mul").unwrap();
    let mir = preprocess(mir).unwrap();
    let mult_id = mir.outputs[0].operation_id;
    let mult_op = mir.operation(mult_id).map_err(|_| anyhow!("missing multiplication operation"))?;

    if let Operation::Multiplication(Multiplication { left: mul_left, right: mul_right, .. }) = mult_op {
        // The left operand is a reference to the input 'a'
        let left_op = assert_input_reference(&mir, *mul_left, "a");
        assert_eq!(*mul_left, left_op.id());
        // Right operand is addition of a + b
        let right_op = mir.operation(*mul_right).map_err(|_| anyhow!("missing right input operation"))?;
        if let Operation::Addition(Addition { left: add_left, right: add_right, .. }) = right_op {
            let _add_left_op = assert_input_reference(&mir, *add_left, "a");
            let add_right_op = assert_input_reference(&mir, *add_right, "b");
            // Left operand of addition is a
            assert_eq!(add_left, mul_left);
            assert_eq!(*add_right, add_right_op.id());
        } else {
            panic!("Invalid right operation {:?}", mul_right);
        }
    } else {
        panic!("Invalid result operation {:?}", mult_op);
    }

    Ok(())
}

#[test]
fn preprocess_unzip() -> Result<(), Error> {
    let mir = read_test_mir("unzip_simple")?;
    let unzip_id = mir.outputs[0].operation_id;
    let unzip_op = get_unzip(&mir, unzip_id)?;
    let unzip_ty = unzip_op.ty.clone();
    let NadaType::Tuple { left_type, right_type } = &unzip_ty else {
        bail!("'unzip' type is not a tuple");
    };
    let input_id = unzip_op.this;
    let input_ty = mir.operation(input_id).unwrap().ty().clone();
    let NadaType::Array { size, inner_type: input_inner_ty } = input_ty else {
        bail!("'unzip' input type is not an array");
    };
    // operations:
    // 2 IdentifierReference
    // Zip operations
    // - New (Array)
    // - for each element in the array
    //   - New (Tuple)
    //   - ArrayAccessor (left brach)
    //   - ArrayAccessor (right branch)
    // Unzip operations
    // - New (Tuple)
    // - New (left Array)
    // - New (right Array)
    // - for each element in the array
    //   - TupleAccessor (left branch)
    //   - TupleAccessor (right branch)
    //   - ArrayAccessor
    let expected_instrunction_count = 6 + 6 * size; // 2 + 1 + size * 3 + 3 + size * 3;
    let unzip_id = unzip_op.id;

    let preprocessed_mir = preprocess(mir)?;
    assert_eq!(preprocessed_mir.operations.len(), expected_instrunction_count);
    // Check that the New Tuple is well-formed
    let new_tuple_op = assert_new(&preprocessed_mir, unzip_id, &unzip_ty, 2)?;

    let tuple_accessor_ty = NadaType::SecretInteger;
    // Traverse the tuple branches
    for (new_array_op_id, tuple_index) in new_tuple_op.elements.iter().zip(vec![TupleIndex::Left, TupleIndex::Right]) {
        // Check the types, it depend on the branch
        let array_ty = match tuple_index {
            TupleIndex::Left => left_type.as_ref(),
            TupleIndex::Right => right_type.as_ref(),
        };
        let new_array_op = assert_new(&preprocessed_mir, *new_array_op_id, array_ty, size)?;
        // Check each inner element in the array
        for (array_index, tuple_accessor_op_id) in new_array_op.elements.iter().enumerate() {
            let tuple_accessor_op = get_tuple_accessor(&preprocessed_mir, *tuple_accessor_op_id)?;
            let array_accessor_op_id = tuple_accessor_op.source;
            // They are references to the elements that input contains. For that they are
            // tuple accessors that refer to the array accessor.
            assert_tuple_accessor(
                &preprocessed_mir,
                *tuple_accessor_op_id,
                &tuple_accessor_ty,
                array_accessor_op_id,
                tuple_index,
            )?;
            assert_array_accessor(
                &preprocessed_mir,
                array_accessor_op_id,
                input_inner_ty.as_ref(),
                input_id,
                array_index,
            )?;
        }
    }
    Ok(())
}

/// Checks that `nada_fn_simple` is inlined properly
#[test]
fn preprocess_function_simple() -> Result<(), Error> {
    let mir = read_test_mir("nada_fn_simple")?;

    let mir = preprocess(mir)?;
    let addition_id = mir.outputs[0].operation_id;
    let addition_op = mir.operation(addition_id).map_err(|_| anyhow!("missing addition operation"))?;
    if let Operation::Addition(Addition { left: result_left, right: result_right, .. }) = addition_op {
        let left_op = mir.operation(*result_left).map_err(|_| anyhow!("missing left input operation"))?;
        let right_op = mir.operation(*result_right).map_err(|_| anyhow!("missing right input operation"))?;
        assert_eq!(*result_left, left_op.id());
        assert_eq!(*result_right, right_op.id());
    } else {
        panic!("Invalid result operation {:?}", addition_op);
    }

    Ok(())
}

#[test]
fn preprocess_function_max() -> Result<(), Error> {
    // (a < b).if_else(b, a)
    let mir = read_test_mir("nada_fn_max")?;
    let mir = preprocess(mir)?;
    let ifelse_id = mir.outputs[0].operation_id;
    let ifelse_op = mir.operation(ifelse_id).map_err(|_| anyhow!("missing ifelse operation"))?;
    if let Operation::IfElse(IfElse { this, arg_0, arg_1, .. }) = ifelse_op {
        // This is a LessThan, arg_0 is the left element of the less than
        let lt_op = mir.operation(*this).map_err(|_| anyhow!("missing less than operation"))?;
        if let Operation::LessThan(LessThan { left, right, .. }) = lt_op {
            assert_eq!(left, arg_1);
            assert_eq!(right, arg_0);
        } else {
            panic!("invalid left operation");
        }
    }
    Ok(())
}

#[test]
fn preprocess_map_simple() -> Result<(), Error> {
    // my_array_1.map(|a| a + my_int)
    let size = 3usize;
    let mir = read_test_mir("map_simple")?;
    let map_id = mir.outputs[0].operation_id;
    let preprocessed_mir = preprocess(mir)?;
    // 2 Input Ref
    // New
    // size x Addition
    // size x ArrayAccessor
    let expected_instrunction_count = 3 + size * 2;
    assert_eq!(preprocessed_mir.operations.len(), expected_instrunction_count);

    let map_ty = NadaType::Array { inner_type: Box::new(NadaType::SecretInteger), size };
    let map_op = assert_new(&preprocessed_mir, map_id, &map_ty, size)?;

    for addition_id in map_op.elements.iter() {
        let addition = get_addition(&preprocessed_mir, *addition_id)?;
        let accessor = get_array_accessor(&preprocessed_mir, addition.left)?;
        let my_array = get_input_reference(&preprocessed_mir, accessor.source)?;
        assert_eq!(&my_array.refers_to, "my_array_1");
        let input = get_input_reference(&preprocessed_mir, addition.right)?;
        assert_eq!(&input.refers_to, "my_int");
    }

    Ok(())
}

#[test]
fn preprocess_reduce_simple() -> Result<(), Error> {
    // my_array_1.reduce(add, 0)
    let size = 3usize;
    let mir = read_test_mir("reduce_simple")?;
    let mut reduce_id = mir.outputs[0].operation_id;
    let preprocessed_mir = preprocess(mir)?;
    // 1 Input Ref
    // 1 Literal Ref
    // size x Addition
    // size x ArrayAccessor
    let expected_instrunction_count = 2 + size * 2; // zip operations
    assert_eq!(preprocessed_mir.operations.len(), expected_instrunction_count);

    for _ in 0..size {
        let addition = get_addition(&preprocessed_mir, reduce_id)?;
        let accessor = get_array_accessor(&preprocessed_mir, addition.right)?;
        let my_array = get_input_reference(&preprocessed_mir, accessor.source)?;
        assert_eq!(&my_array.refers_to, "my_array_1");
        reduce_id = addition.left;
    }

    Ok(())
}

#[test]
// fn add() {
//    add();
// }
// add()
fn recursion_1st_operation() -> Result<(), Error> {
    let mut mir = ProgramMIR::build();
    mir.add_input("my_int", NadaType::SecretInteger, "party_1");
    let mut id_generator = OperationIdGenerator::default();
    let mut add_fn = NadaFunction::build("add", NadaType::SecretInteger, id_generator.next_id());
    let add_fn_id = add_fn.id;
    add_fn.add_operation(NadaFunctionCall::build(add_fn_id, vec![], NadaType::SecretInteger, id_generator.next_id()));
    mir.add_function(add_fn);
    let return_operation_id =
        mir.add_operation(NadaFunctionCall::build(add_fn_id, vec![], NadaType::SecretInteger, id_generator.next_id()));
    mir.add_output("output", return_operation_id, NadaType::SecretInteger, "party_1");
    let preprocessed_mir = preprocess(mir);
    assert!(matches!(
        preprocessed_mir,
        Err(MIRPreprocessorError::Malformed(MIRProgramMalformed::FunctionRecursion(_)))
    ));
    Ok(())
}

#[test]
// fn add() {
//    // something
//    add();
// }
// add()
fn recursion_2nd_operation() -> Result<(), Error> {
    let mut mir = ProgramMIR::build();
    mir.add_input("my_int", NadaType::SecretInteger, "party_1");
    let mut id_generator = OperationIdGenerator::default();
    let mut add_fn = NadaFunction::build("add", NadaType::SecretInteger, id_generator.next_id());
    let add_fn_id = add_fn.id;
    add_fn.add_operation(InputReference::build("my_input", NadaType::SecretInteger, id_generator.next_id()));
    add_fn.add_operation(NadaFunctionCall::build(add_fn_id, vec![], NadaType::SecretInteger, id_generator.next_id()));
    mir.add_function(add_fn);
    let return_operation_id =
        mir.add_operation(NadaFunctionCall::build(add_fn_id, vec![], NadaType::SecretInteger, id_generator.next_id()));
    mir.add_output("output", return_operation_id, NadaType::SecretInteger, "party_1");
    let preprocessed_mir = preprocess(mir);
    assert!(matches!(
        preprocessed_mir,
        Err(MIRPreprocessorError::Malformed(MIRProgramMalformed::FunctionRecursion(_)))
    ));
    Ok(())
}

#[test]
// fn add1() {
//    add2();
// }
// fn add2() {
//    // something
//    add1();
// }
// add1()
fn deep_recursion() -> Result<(), Error> {
    let mut mir = ProgramMIR::build();
    mir.add_input("my_int", NadaType::SecretInteger, "party_1");
    let mut id_generator = OperationIdGenerator::default();
    let mut add1_fn = NadaFunction::build("add1", NadaType::SecretInteger, id_generator.next_id());
    let add1_fn_id = add1_fn.id;
    let mut add2_fn = NadaFunction::build("add2", NadaType::SecretInteger, id_generator.next_id());
    let add2_fn_id = add2_fn.id;
    add1_fn.add_operation(NadaFunctionCall::build(add2_fn_id, vec![], NadaType::SecretInteger, id_generator.next_id()));
    add2_fn.add_operation(InputReference::build("my_input", NadaType::SecretInteger, id_generator.next_id()));
    add2_fn.add_operation(NadaFunctionCall::build(add1_fn_id, vec![], NadaType::SecretInteger, id_generator.next_id()));
    mir.add_function(add1_fn);
    mir.add_function(add2_fn);
    let return_operation_id =
        mir.add_operation(NadaFunctionCall::build(add1_fn_id, vec![], NadaType::SecretInteger, id_generator.next_id()));
    mir.add_output("output", return_operation_id, NadaType::SecretInteger, "party_1");
    let preprocessed_mir = preprocess(mir);
    assert!(matches!(
        preprocessed_mir,
        Err(MIRPreprocessorError::Malformed(MIRProgramMalformed::FunctionRecursion(_)))
    ));
    Ok(())
}

#[test]
// fn add1() {
//    add2();
//    add2();
// }
// fn add2() {
//    // something
// }
// add1()
fn no_recursion() -> Result<(), Error> {
    let mut mir = ProgramMIR::build();
    mir.add_input("my_int", NadaType::SecretInteger, "party_1");
    let mut id_generator = OperationIdGenerator::default();
    let mut add2_fn = NadaFunction::build("add2", NadaType::SecretInteger, id_generator.next_id());
    let add2_fn_id = add2_fn.id;
    add2_fn.add_operation(InputReference::build("my_input", NadaType::SecretInteger, id_generator.next_id()));
    mir.add_function(add2_fn);

    let mut add1_fn = NadaFunction::build("add1", NadaType::SecretInteger, id_generator.next_id());
    let add1_fn_id = add1_fn.id;
    add1_fn.add_operation(NadaFunctionCall::build(add2_fn_id, vec![], NadaType::SecretInteger, id_generator.next_id()));
    add1_fn.add_operation(NadaFunctionCall::build(add2_fn_id, vec![], NadaType::SecretInteger, id_generator.next_id()));
    mir.add_function(add1_fn);

    let return_operation_id =
        mir.add_operation(NadaFunctionCall::build(add1_fn_id, vec![], NadaType::SecretInteger, id_generator.next_id()));
    mir.add_output("output", return_operation_id, NadaType::SecretInteger, "party_1");

    println!("{mir:#?}");
    let preprocessed_mir = preprocess(mir);
    assert!(preprocessed_mir.is_ok());
    Ok(())
}

#[test]
// fn add1() {
//    // anything
//    my_array_1.map(add2)
//    // anything
// }
// fn add2() {
//    // anything
//    my_array_1.map(add1)
//    // anything
// }
// my_array_1.map(add1)
fn map_recursion() -> Result<(), Error> {
    let mut mir = ProgramMIR::build();
    let array_type = NadaType::Array { inner_type: Box::new(NadaType::SecretInteger), size: 5 };
    mir.add_input("my_array_1", array_type.clone(), "party_1");
    let mut id_generator = OperationIdGenerator::default();
    let mut add1_fn = NadaFunction::build("add1", NadaType::SecretInteger, id_generator.next_id());
    let add1_fn_id = add1_fn.id;
    let mut add2_fn = NadaFunction::build("add2", NadaType::SecretInteger, id_generator.next_id());
    let add2_fn_id = add2_fn.id;
    let input_ref_id =
        add2_fn.add_operation(InputReference::build("my_array_1", array_type.clone(), id_generator.next_id()));
    add2_fn.add_operation(Map::build(add1_fn_id, input_ref_id, array_type.clone(), id_generator.next_id()));
    let input_ref_id =
        add1_fn.add_operation(InputReference::build("my_array_1", array_type.clone(), id_generator.next_id()));
    add1_fn.add_operation(Map::build(add2_fn_id, input_ref_id, array_type.clone(), id_generator.next_id()));
    mir.add_function(add1_fn);
    mir.add_function(add2_fn);
    let input_ref_id =
        mir.add_operation(InputReference::build("my_array_1", array_type.clone(), id_generator.next_id()));
    let return_operation_id =
        mir.add_operation(Map::build(add1_fn_id, input_ref_id, array_type.clone(), id_generator.next_id()));
    mir.add_output("output", return_operation_id, array_type.clone(), "party_1");
    let preprocessed_mir = preprocess(mir);
    assert!(matches!(
        preprocessed_mir,
        Err(MIRPreprocessorError::Malformed(MIRProgramMalformed::FunctionRecursion(_)))
    ));
    Ok(())
}

#[test]
// fn add1() {
//    // anything
//    my_array_1.reduce(add2)
//    // anything
// }
// fn add2() {
//    // anything
//    my_array_1.reduce(add1)
//    // anything
// }
// my_array_1.map(add1)
fn reduce_recursion() -> Result<(), Error> {
    let mut mir = ProgramMIR::build();
    let array_type = NadaType::Array { inner_type: Box::new(NadaType::SecretInteger), size: 5 };
    mir.add_input("my_int", NadaType::SecretInteger, "party_1");
    mir.add_input("my_array_1", array_type.clone(), "party_1");
    let mut id_generator = OperationIdGenerator::default();
    let mut add1_fn = NadaFunction::build("add1", NadaType::SecretInteger, id_generator.next_id());
    let add1_fn_id = add1_fn.id;
    let mut add2_fn = NadaFunction::build("add2", NadaType::SecretInteger, id_generator.next_id());
    let add2_fn_id = add2_fn.id;
    let initial_ref_id =
        add2_fn.add_operation(InputReference::build("my_int", array_type.clone(), id_generator.next_id()));
    let input_ref_id =
        add2_fn.add_operation(InputReference::build("my_array_1", array_type.clone(), id_generator.next_id()));
    add2_fn.add_operation(Reduce::build(
        add1_fn_id,
        initial_ref_id,
        input_ref_id,
        NadaType::SecretInteger,
        id_generator.next_id(),
    ));
    let initial_ref_id =
        add1_fn.add_operation(InputReference::build("my_int", array_type.clone(), id_generator.next_id()));
    let input_ref_id =
        add1_fn.add_operation(InputReference::build("my_array_1", array_type.clone(), id_generator.next_id()));
    add1_fn.add_operation(Reduce::build(
        add2_fn_id,
        initial_ref_id,
        input_ref_id,
        NadaType::SecretInteger,
        id_generator.next_id(),
    ));
    mir.add_function(add1_fn);
    mir.add_function(add2_fn);
    let input_ref_id =
        mir.add_operation(InputReference::build("my_array_1", array_type.clone(), id_generator.next_id()));
    let return_operation_id =
        mir.add_operation(Map::build(add1_fn_id, input_ref_id, array_type.clone(), id_generator.next_id()));
    mir.add_output("output", return_operation_id, array_type.clone(), "party_1");
    let preprocessed_mir = preprocess(mir);
    assert!(matches!(
        preprocessed_mir,
        Err(MIRPreprocessorError::Malformed(MIRProgramMalformed::FunctionRecursion(_)))
    ));
    Ok(())
}
