use anyhow::Error;

use nada_compiler_backend::mir::Operation as MIROperation;
use test_programs::PROGRAMS;

use crate::mir2bytecode::MIR2Bytecode;

#[test]
#[allow(clippy::indexing_slicing)]
fn input_integer() -> Result<(), Error> {
    let mir = PROGRAMS.mir("input_integer")?;
    let plan = MIR2Bytecode::create_plan(&mir)?;
    assert_eq!(plan.len(), 1);
    assert!(matches!(plan[0], MIROperation::InputReference(_)));
    Ok(())
}

#[test]
#[allow(clippy::indexing_slicing)]
fn input_array() -> Result<(), Error> {
    let mir = PROGRAMS.mir("input_array")?;
    let plan = MIR2Bytecode::create_plan(&mir)?;
    assert_eq!(plan.len(), 1);
    assert!(matches!(plan[0], MIROperation::InputReference(_)));
    Ok(())
}

#[test]
// my_int1 + my_int2
#[allow(clippy::indexing_slicing)]
fn plan_addition_simple() -> Result<(), Error> {
    let mir = PROGRAMS.mir("addition_simple")?;
    let plan = MIR2Bytecode::create_plan(&mir)?;
    assert_eq!(plan.len(), 3);
    assert!(matches!(plan[0], MIROperation::InputReference(_)));
    assert!(matches!(plan[1], MIROperation::InputReference(_)));
    assert!(matches!(plan[2], MIROperation::Addition(_)));
    Ok(())
}

#[test]
// ((A * B) + C + D) * (E * (F + G)) + (A * B * (C + D) + E) * F + (A + (B * (C + (D * (E + F)))))
#[allow(clippy::indexing_slicing)]
fn plan_complex_operation_mix() -> Result<(), Error> {
    let mir = PROGRAMS.mir("complex_operation_mix")?;
    let plan = MIR2Bytecode::create_plan(&mir)?;
    assert_eq!(plan.len(), 25);
    assert!(matches!(plan[0], MIROperation::InputReference(_)));
    assert!(matches!(plan[1], MIROperation::InputReference(_)));
    assert!(matches!(plan[2], MIROperation::InputReference(_)));
    assert!(matches!(plan[3], MIROperation::InputReference(_)));
    assert!(matches!(plan[4], MIROperation::InputReference(_)));
    assert!(matches!(plan[5], MIROperation::InputReference(_)));
    assert!(matches!(plan[6], MIROperation::InputReference(_)));
    assert!(matches!(plan[7], MIROperation::Multiplication(_)));
    assert!(matches!(plan[8], MIROperation::Addition(_)));
    assert!(matches!(plan[9], MIROperation::Addition(_)));
    assert!(matches!(plan[10], MIROperation::Addition(_)));
    assert!(matches!(plan[11], MIROperation::Multiplication(_)));
    assert!(matches!(plan[12], MIROperation::Multiplication(_)));
    assert!(matches!(plan[13], MIROperation::Multiplication(_)));
    assert!(matches!(plan[14], MIROperation::Addition(_)));
    assert!(matches!(plan[15], MIROperation::Multiplication(_)));
    assert!(matches!(plan[16], MIROperation::Addition(_)));
    assert!(matches!(plan[17], MIROperation::Multiplication(_)));
    assert!(matches!(plan[18], MIROperation::Addition(_)));
    assert!(matches!(plan[19], MIROperation::Addition(_)));
    assert!(matches!(plan[20], MIROperation::Multiplication(_)));
    assert!(matches!(plan[21], MIROperation::Addition(_)));
    assert!(matches!(plan[22], MIROperation::Multiplication(_)));
    assert!(matches!(plan[23], MIROperation::Addition(_)));
    assert!(matches!(plan[24], MIROperation::Addition(_)));
    Ok(())
}
