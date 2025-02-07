use anyhow::{bail, Error};
use nada_compiler_backend::mir::{
    Addition, ArrayAccessor, GreaterOrEqualThan, GreaterThan, InputReference, LessOrEqualThan, LessThan, New, Not,
    Operation, OperationId, ProgramMIR, TupleAccessor, TupleIndex, Unzip, Zip,
};
use nada_value::NadaType;

macro_rules! get_operation {
    ($($o:ident),+) => {
        $(
        paste::item! {
            pub(crate) fn [<get_$o:snake>](mir: &ProgramMIR, id: OperationId) -> Result<&$o, Error> {
                let Operation::$o(o) = mir.operation(id).unwrap() else {
                    bail!("'{}' not found", stringify!($o));
                };
                Ok(o)
            }
        }
        )+
    };
}

get_operation!(
    Addition,
    ArrayAccessor,
    GreaterThan,
    GreaterOrEqualThan,
    InputReference,
    LessOrEqualThan,
    LessThan,
    New,
    Not,
    TupleAccessor,
    Unzip,
    Zip
);

pub(crate) fn assert_new<'m>(
    mir: &'m ProgramMIR,
    id: OperationId,
    ty: &NadaType,
    size: usize,
) -> Result<&'m New, Error> {
    let new_op = get_new(mir, id)?;
    assert_eq!(new_op.id, id);
    assert_eq!(&new_op.ty, ty);
    assert_eq!(new_op.elements.len(), size);
    Ok(new_op)
}

pub(crate) fn assert_array_accessor<'m>(
    mir: &'m ProgramMIR,
    id: OperationId,
    ty: &NadaType,
    source: OperationId,
    index: usize,
) -> Result<&'m ArrayAccessor, Error> {
    let accessor = get_array_accessor(mir, id)?;
    assert_eq!(accessor.id, id);
    assert_eq!(&accessor.ty, ty);
    assert_eq!(accessor.source, source);
    assert_eq!(accessor.index, index);
    Ok(accessor)
}

pub(crate) fn assert_tuple_accessor<'m>(
    mir: &'m ProgramMIR,
    id: OperationId,
    ty: &NadaType,
    source: OperationId,
    index: TupleIndex,
) -> Result<&'m TupleAccessor, Error> {
    let accessor = get_tuple_accessor(mir, id)?;
    assert_eq!(accessor.id, id);
    assert_eq!(&accessor.ty, ty);
    assert_eq!(accessor.source, source);
    assert_eq!(accessor.index, index);
    Ok(accessor)
}

pub(crate) fn assert_not(mir: &ProgramMIR, id: OperationId) -> Result<&Not, Error> {
    let not_op = get_not(mir, id)?;
    assert_eq!(not_op.id, id);
    Ok(not_op)
}

pub(crate) fn assert_less_than(
    mir: &ProgramMIR,
    id: OperationId,
    left: OperationId,
    right: OperationId,
) -> Result<&LessThan, Error> {
    let operation = get_less_than(mir, id)?;
    assert_eq!(operation.id, id);
    assert_eq!(operation.left, left);
    assert_eq!(operation.right, right);
    Ok(operation)
}
