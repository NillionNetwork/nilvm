use crate::{
    Input, Literal, NadaFunction, NadaFunctionArg, Operation, OperationId, OperationMap, Output, Party, ProgramMIR,
    SourceRef, SourceRefIndex, TupleIndex,
};
use mir_proto::nillion::nada::{mir::v1 as proto_mir, operations::v1 as proto_op, types::v1 as proto_ty};
use nada_type::{HashableIndexMap, IndexMap, NadaType};
pub use prost::Message;
use std::{
    collections::{BTreeMap, HashMap},
    hash::Hash,
};

pub use mir_proto::nillion::nada::mir::v1::ProgramMir as ProtoProgramMIR;

#[derive(Debug, thiserror::Error)]
#[error("protobuf parsing error: {0}")]
pub struct ProtoError(pub &'static str);

/// A trait that allows converting a trait from/into protobuf.
pub trait ConvertProto: Sized {
    /// The protobuf type that represents this type.
    type ProtoType;

    /// Convert this type into protobuf.
    fn into_proto(self) -> Self::ProtoType;

    /// Try to construct an instance from a protobuf type.
    fn try_from_proto(model: Self::ProtoType) -> Result<Self, ProtoError>;

    fn try_decode(bytes: &[u8]) -> Result<Self, ProtoError>
    where
        Self::ProtoType: Message + Default,
    {
        let model = Self::ProtoType::decode(bytes).map_err(|_| ProtoError("protobuf decoding failed"))?;
        model.try_into_rust()
    }
}

/// Try to convert a protobuf model into a rust type.
pub trait TryIntoRust<T> {
    /// Try to convert this protobuf model into a rust type.
    fn try_into_rust(self) -> Result<T, ProtoError>;
}

impl<T, U> TryIntoRust<T> for U
where
    T: ConvertProto<ProtoType = U>,
{
    fn try_into_rust(self) -> Result<T, ProtoError> {
        T::try_from_proto(self)
    }
}

/// A marker trait that indicates a type's protobuf model is the same as the rust one.
///
/// This allows always using `ConvertProto` for a type without having to know if the rust type is
/// the same as the protobuf one.
pub trait TransparentProto {}

impl<T: TransparentProto> ConvertProto for T {
    type ProtoType = T;

    fn into_proto(self) -> Self::ProtoType {
        self
    }

    fn try_from_proto(model: Self::ProtoType) -> Result<Self, ProtoError> {
        Ok(model)
    }
}

impl TransparentProto for String {}

impl ConvertProto for Box<NadaType> {
    type ProtoType = Box<proto_ty::NadaType>;

    fn into_proto(self) -> Self::ProtoType {
        Box::new((*self).into_proto())
    }

    fn try_from_proto(model: Self::ProtoType) -> Result<Self, ProtoError> {
        Ok(Box::new(NadaType::try_from_proto(*model)?))
    }
}

impl<T: ConvertProto> ConvertProto for Vec<T> {
    type ProtoType = Vec<T::ProtoType>;

    fn into_proto(self) -> Self::ProtoType {
        self.into_iter().map(T::into_proto).collect()
    }

    fn try_from_proto(model: Self::ProtoType) -> Result<Self, ProtoError> {
        model.into_iter().map(T::try_from_proto).collect()
    }
}

impl<K, V> ConvertProto for BTreeMap<K, V>
where
    K: ConvertProto + Eq + Ord,
    K::ProtoType: Eq + Hash,
    V: ConvertProto,
{
    type ProtoType = HashMap<K::ProtoType, V::ProtoType>;

    fn into_proto(self) -> Self::ProtoType {
        self.into_iter().map(|(k, v)| (k.into_proto(), v.into_proto())).collect::<HashMap<K::ProtoType, V::ProtoType>>()
    }

    fn try_from_proto(model: Self::ProtoType) -> Result<Self, ProtoError> {
        model
            .into_iter()
            .map(|(k, v)| Ok((K::try_from_proto(k)?, V::try_from_proto(v)?)))
            .collect::<Result<BTreeMap<K, V>, ProtoError>>()
    }
}

fn operation_map_to_proto(map: OperationMap) -> Vec<proto_mir::OperationMapEntry> {
    map.into_iter()
        .map(|(k, v)| proto_mir::OperationMapEntry { id: k.into_proto(), operation: Some(v.into_proto()) })
        .collect()
}

fn try_operations_proto_to_rust(model: Vec<proto_mir::OperationMapEntry>) -> Result<OperationMap, ProtoError> {
    model
        .into_iter()
        .map(|entry| {
            Ok((
                entry.id.try_into_rust()?,
                entry.operation.ok_or(ProtoError("operation value not set"))?.try_into_rust()?,
            ))
        })
        .collect::<Result<OperationMap, ProtoError>>()
}

impl ConvertProto for ProgramMIR {
    type ProtoType = proto_mir::ProgramMir;

    fn into_proto(self) -> Self::ProtoType {
        let operations = operation_map_to_proto(self.operations);
        proto_mir::ProgramMir {
            functions: self.functions.into_proto(),
            parties: self.parties.into_proto(),
            inputs: self.inputs.into_proto(),
            literals: self.literals.into_proto(),
            outputs: self.outputs.into_proto(),
            operations,
            source_files: self.source_files.into_proto(),
            source_refs: self.source_refs.into_proto(),
        }
    }

    fn try_from_proto(model: Self::ProtoType) -> Result<Self, ProtoError> {
        let operations = try_operations_proto_to_rust(model.operations)?;
        Ok(ProgramMIR {
            functions: model.functions.try_into_rust()?,
            parties: model.parties.try_into_rust()?,
            inputs: model.inputs.try_into_rust()?,
            literals: model.literals.try_into_rust()?,
            outputs: model.outputs.try_into_rust()?,
            operations,
            source_files: model.source_files.try_into_rust()?,
            source_refs: model.source_refs.try_into_rust()?,
        })
    }
}

impl ConvertProto for NadaFunction {
    type ProtoType = proto_mir::NadaFunction;

    fn into_proto(self) -> Self::ProtoType {
        let operations = operation_map_to_proto(self.operations);
        proto_mir::NadaFunction {
            id: self.id.into_proto(),
            name: self.name,
            operations,
            return_operation_id: self.return_operation_id.into_proto(),
            return_type: Some(self.return_type.into_proto()),
            args: self.args.into_proto(),
            source_ref_index: self.source_ref_index.into_proto(),
        }
    }

    fn try_from_proto(model: Self::ProtoType) -> Result<Self, ProtoError> {
        let operations = try_operations_proto_to_rust(model.operations)?;
        Ok(NadaFunction {
            id: model.id.try_into_rust()?,
            name: model.name,
            operations,
            return_operation_id: model.return_operation_id.try_into_rust()?,
            return_type: model.return_type.ok_or(ProtoError("return type not set"))?.try_into_rust()?,
            args: model.args.try_into_rust()?,
            source_ref_index: model.source_ref_index.try_into_rust()?,
        })
    }
}

impl ConvertProto for NadaFunctionArg {
    type ProtoType = proto_mir::NadaFunctionArg;

    fn into_proto(self) -> Self::ProtoType {
        proto_mir::NadaFunctionArg {
            name: self.name,
            r#type: Some(self.ty.into_proto()),
            source_ref_index: self.source_ref_index.into_proto(),
        }
    }

    fn try_from_proto(model: Self::ProtoType) -> Result<Self, ProtoError> {
        Ok(NadaFunctionArg {
            name: model.name,
            ty: model.r#type.ok_or(ProtoError("type not set"))?.try_into_rust()?,
            source_ref_index: model.source_ref_index.try_into_rust()?,
        })
    }
}

impl ConvertProto for Party {
    type ProtoType = proto_mir::Party;

    fn into_proto(self) -> Self::ProtoType {
        proto_mir::Party { name: self.name, source_ref_index: self.source_ref_index.into_proto() }
    }

    fn try_from_proto(model: Self::ProtoType) -> Result<Self, ProtoError> {
        Ok(Party { name: model.name, source_ref_index: model.source_ref_index.try_into_rust()? })
    }
}

impl ConvertProto for Input {
    type ProtoType = proto_mir::Input;

    fn into_proto(self) -> Self::ProtoType {
        proto_mir::Input {
            r#type: Some(self.ty.into_proto()),
            party: self.party,
            name: self.name,
            doc: self.doc,
            source_ref_index: self.source_ref_index.into_proto(),
        }
    }

    fn try_from_proto(model: Self::ProtoType) -> Result<Self, ProtoError> {
        Ok(Input {
            ty: model.r#type.ok_or(ProtoError("type not set"))?.try_into_rust()?,
            party: model.party,
            name: model.name,
            doc: model.doc,
            source_ref_index: model.source_ref_index.try_into_rust()?,
        })
    }
}

impl ConvertProto for Output {
    type ProtoType = proto_mir::Output;

    fn into_proto(self) -> Self::ProtoType {
        proto_mir::Output {
            r#type: Some(self.ty.into_proto()),
            party: self.party,
            name: self.name,
            source_ref_index: self.source_ref_index.into_proto(),
            operation_id: self.operation_id.into_proto(),
        }
    }

    fn try_from_proto(model: Self::ProtoType) -> Result<Self, ProtoError> {
        Ok(Output {
            ty: model.r#type.ok_or(ProtoError("type not set"))?.try_into_rust()?,
            party: model.party,
            name: model.name,
            source_ref_index: model.source_ref_index.try_into_rust()?,
            operation_id: model.operation_id.try_into_rust()?,
        })
    }
}

impl ConvertProto for Operation {
    type ProtoType = proto_op::Operation;

    fn into_proto(self) -> Self::ProtoType {
        match self {
            Operation::Reduce(o) => op_rust_to_proto::reduce(o),
            Operation::Map(o) => op_rust_to_proto::map(o),
            Operation::Unzip(o) => op_rust_to_proto::unzip(o),
            Operation::Zip(o) => op_rust_to_proto::zip(o),
            Operation::Addition(o) => op_rust_to_proto::addition(o),
            Operation::Subtraction(o) => op_rust_to_proto::subtraction(o),
            Operation::Multiplication(o) => op_rust_to_proto::multiplication(o),
            Operation::LessThan(o) => op_rust_to_proto::less_than(o),
            Operation::LessOrEqualThan(o) => op_rust_to_proto::less_or_equal_than(o),
            Operation::GreaterThan(o) => op_rust_to_proto::greater_than(o),
            Operation::GreaterOrEqualThan(o) => op_rust_to_proto::greater_or_equal_than(o),
            Operation::PublicOutputEquality(o) => op_rust_to_proto::public_output_equality(o),
            Operation::Equals(o) => op_rust_to_proto::equals(o),
            Operation::Cast(o) => op_rust_to_proto::cast(o),
            Operation::InputReference(o) => op_rust_to_proto::input_reference(o),
            Operation::LiteralReference(o) => op_rust_to_proto::literal_reference(o),
            Operation::NadaFunctionArgRef(o) => op_rust_to_proto::nada_function_arg_ref(o),
            Operation::Modulo(o) => op_rust_to_proto::modulo(o),
            Operation::Power(o) => op_rust_to_proto::power(o),
            Operation::Division(o) => op_rust_to_proto::division(o),
            Operation::NadaFunctionCall(o) => op_rust_to_proto::nada_function_call(o),
            Operation::ArrayAccessor(o) => op_rust_to_proto::array_accessor(o),
            Operation::TupleAccessor(o) => op_rust_to_proto::tuple_accessor(o),
            Operation::New(o) => op_rust_to_proto::new(o),
            Operation::Random(o) => op_rust_to_proto::random(o),
            Operation::IfElse(o) => op_rust_to_proto::if_else(o),
            Operation::Reveal(o) => op_rust_to_proto::reveal(o),
            Operation::PublicKeyDerive(o) => op_rust_to_proto::public_key_derive(o),
            Operation::Not(o) => op_rust_to_proto::not(o),
            Operation::LeftShift(o) => op_rust_to_proto::left_shift(o),
            Operation::RightShift(o) => op_rust_to_proto::right_shift(o),
            Operation::TruncPr(o) => op_rust_to_proto::trunc_pr(o),
            Operation::InnerProduct(o) => op_rust_to_proto::inner_product(o),
            Operation::NotEquals(o) => op_rust_to_proto::not_equals(o),
            Operation::BooleanAnd(o) => op_rust_to_proto::boolean_and(o),
            Operation::BooleanOr(o) => op_rust_to_proto::boolean_or(o),
            Operation::BooleanXor(o) => op_rust_to_proto::boolean_xor(o),
            Operation::EcdsaSign(o) => op_rust_to_proto::ecdsa_sign(o),
            Operation::EddsaSign(o) => op_rust_to_proto::eddsa_sign(o),
        }
    }

    fn try_from_proto(operation: Self::ProtoType) -> Result<Self, ProtoError> {
        use proto_op::{operation::Operation as ProtoOperation, BinaryOperationVariant, UnaryOperationVariant};
        let operation_variant = operation.operation.clone().ok_or(ProtoError("operation not set"))?;
        match operation_variant {
            ProtoOperation::Binary(binary) => match BinaryOperationVariant::try_from(binary.variant)
                .map_err(|_| ProtoError("Can't parse binary variant into enum"))?
            {
                BinaryOperationVariant::Addition => op_proto_to_rust::addition(operation, binary),
                BinaryOperationVariant::Subtraction => op_proto_to_rust::subtraction(operation, binary),
                BinaryOperationVariant::Multiplication => op_proto_to_rust::multiplication(operation, binary),
                BinaryOperationVariant::LessThan => op_proto_to_rust::less_than(operation, binary),
                BinaryOperationVariant::LessEq => op_proto_to_rust::less_or_equal_than(operation, binary),
                BinaryOperationVariant::GreaterThan => op_proto_to_rust::greater_than(operation, binary),
                BinaryOperationVariant::GreaterEq => op_proto_to_rust::greater_or_equal_than(operation, binary),
                BinaryOperationVariant::EqualsPublicOutput => {
                    op_proto_to_rust::public_output_equality(operation, binary)
                }
                BinaryOperationVariant::Equals => op_proto_to_rust::equals(operation, binary),
                BinaryOperationVariant::Modulo => op_proto_to_rust::modulo(operation, binary),
                BinaryOperationVariant::Power => op_proto_to_rust::power(operation, binary),
                BinaryOperationVariant::Division => op_proto_to_rust::division(operation, binary),
                BinaryOperationVariant::LeftShift => op_proto_to_rust::left_shift(operation, binary),
                BinaryOperationVariant::RightShift => op_proto_to_rust::right_shift(operation, binary),
                BinaryOperationVariant::TruncPr => op_proto_to_rust::trunc_pr(operation, binary),
                BinaryOperationVariant::NotEquals => op_proto_to_rust::not_equals(operation, binary),
                BinaryOperationVariant::BoolAnd => op_proto_to_rust::boolean_and(operation, binary),
                BinaryOperationVariant::BoolOr => op_proto_to_rust::boolean_or(operation, binary),
                BinaryOperationVariant::BoolXor => op_proto_to_rust::boolean_xor(operation, binary),
                BinaryOperationVariant::Zip => op_proto_to_rust::zip(operation, binary),
                BinaryOperationVariant::InnerProduct => op_proto_to_rust::inner_product(operation, binary),
                BinaryOperationVariant::EcdsaSign => op_proto_to_rust::ecdsa_sign(operation, binary),
                BinaryOperationVariant::EddsaSign => op_proto_to_rust::eddsa_sign(operation, binary),
            },
            ProtoOperation::Unary(unary) => match UnaryOperationVariant::try_from(unary.variant)
                .map_err(|_| ProtoError("Can't parse binary variant into enum"))?
            {
                UnaryOperationVariant::Unzip => op_proto_to_rust::unzip(operation, unary),
                UnaryOperationVariant::Reveal => op_proto_to_rust::reveal(operation, unary),
                UnaryOperationVariant::Not => op_proto_to_rust::not(operation, unary),
                UnaryOperationVariant::PublicKeyDerive => op_proto_to_rust::public_key_derive(operation, unary),
            },
            ProtoOperation::Ifelse(o) => op_proto_to_rust::if_else(operation, o),
            ProtoOperation::Random(_) => op_proto_to_rust::random(operation),
            ProtoOperation::InputRef(o) => op_proto_to_rust::input_reference(operation, o),
            ProtoOperation::LiteralRef(o) => op_proto_to_rust::literal_reference(operation, o),
            ProtoOperation::ArgRef(o) => op_proto_to_rust::arg_reference(operation, o),
            ProtoOperation::Map(o) => op_proto_to_rust::map(operation, o),
            ProtoOperation::Reduce(o) => op_proto_to_rust::reduce(operation, o),
            ProtoOperation::New(o) => op_proto_to_rust::new(operation, o),
            ProtoOperation::ArrayAccessor(o) => op_proto_to_rust::array_accessor(operation, o),
            ProtoOperation::TupleAccessor(o) => op_proto_to_rust::tuple_accessor(operation, o),
            ProtoOperation::NtupleAccessor(o) => op_proto_to_rust::ntuple_accessor(operation, o),
            ProtoOperation::ObjectAccessor(o) => op_proto_to_rust::object_accessor(operation, o),
            ProtoOperation::Cast(o) => op_proto_to_rust::cast(operation, o),
        }
    }
}

mod op_rust_to_proto {
    use crate::{
        proto::ConvertProto, Addition, ArrayAccessor, BooleanAnd, BooleanOr, BooleanXor, Cast, Division, EcdsaSign,
        EddsaSign, Equals, GreaterOrEqualThan, GreaterThan, IfElse, InnerProduct, InputReference, LeftShift,
        LessOrEqualThan, LessThan, LiteralReference, Map, Modulo, Multiplication, NadaFunctionArgRef, NadaFunctionCall,
        New, Not, NotEquals, Power, PublicKeyDerive, PublicOutputEquality, Random, Reduce, Reveal, RightShift,
        Subtraction, TruncPr, TupleAccessor, Unzip, Zip,
    };
    use mir_proto::nillion::nada::operations::v1::{
        operation::Operation as OperationVariant, ArrayAccessor as ArrayAccessorOperation, BinaryOperation,
        BinaryOperationVariant, CastOperation, IfElseOperation, InputReference as InputReferenceOperation,
        LiteralReference as LiteralReferenceOperation, MapOperation, NadaFunctionArgRef as NadaFunctionArgRefOperation,
        NewOperation, Operation, ReduceOperation, TupleAccessor as TupleAccessorOperation, UnaryOperation,
        UnaryOperationVariant,
    };

    pub(crate) fn reduce(reduce: Reduce) -> Operation {
        Operation {
            id: reduce.id.into_proto(),
            r#type: Some(reduce.ty.into_proto()),
            source_ref_index: reduce.source_ref_index.into_proto(),
            operation: Some(OperationVariant::Reduce(ReduceOperation {
                r#fn: reduce.function_id.into_proto(),
                child: reduce.inner.into_proto(),
                initial: reduce.initial.into_proto(),
            })),
        }
    }

    pub(crate) fn map(map: Map) -> Operation {
        Operation {
            id: map.id.into_proto(),
            r#type: Some(map.ty.into_proto()),
            source_ref_index: map.source_ref_index.into_proto(),
            operation: Some(OperationVariant::Map(MapOperation {
                r#fn: map.function_id.into_proto(),
                child: map.inner.into_proto(),
            })),
        }
    }

    pub(crate) fn unzip(unzip: Unzip) -> Operation {
        Operation {
            id: unzip.id.into_proto(),
            r#type: Some(unzip.ty.into_proto()),
            source_ref_index: unzip.source_ref_index.into_proto(),
            operation: Some(OperationVariant::Unary(UnaryOperation {
                variant: UnaryOperationVariant::Unzip as i32,
                this: unzip.this.into_proto(),
            })),
        }
    }

    pub(crate) fn zip(zip: Zip) -> Operation {
        Operation {
            id: zip.id.into_proto(),
            r#type: Some(zip.ty.into_proto()),
            source_ref_index: zip.source_ref_index.into_proto(),
            operation: Some(OperationVariant::Binary(BinaryOperation {
                variant: BinaryOperationVariant::Zip as i32,
                left: zip.left.into_proto(),
                right: zip.right.into_proto(),
            })),
        }
    }

    pub(crate) fn addition(addition: Addition) -> Operation {
        Operation {
            id: addition.id.into_proto(),
            r#type: Some(addition.ty.into_proto()),
            source_ref_index: addition.source_ref_index.into_proto(),
            operation: Some(OperationVariant::Binary(BinaryOperation {
                variant: BinaryOperationVariant::Addition as i32,
                left: addition.left.into_proto(),
                right: addition.right.into_proto(),
            })),
        }
    }

    pub(crate) fn subtraction(subtraction: Subtraction) -> Operation {
        Operation {
            id: subtraction.id.into_proto(),
            r#type: Some(subtraction.ty.into_proto()),
            source_ref_index: subtraction.source_ref_index.into_proto(),
            operation: Some(OperationVariant::Binary(BinaryOperation {
                variant: BinaryOperationVariant::Subtraction as i32,
                left: subtraction.left.into_proto(),
                right: subtraction.right.into_proto(),
            })),
        }
    }

    pub(crate) fn multiplication(multiplication: Multiplication) -> Operation {
        Operation {
            id: multiplication.id.into_proto(),
            r#type: Some(multiplication.ty.into_proto()),
            source_ref_index: multiplication.source_ref_index.into_proto(),
            operation: Some(OperationVariant::Binary(BinaryOperation {
                variant: BinaryOperationVariant::Multiplication as i32,
                left: multiplication.left.into_proto(),
                right: multiplication.right.into_proto(),
            })),
        }
    }

    pub(crate) fn less_than(less_than: LessThan) -> Operation {
        Operation {
            id: less_than.id.into_proto(),
            r#type: Some(less_than.ty.into_proto()),
            source_ref_index: less_than.source_ref_index.into_proto(),
            operation: Some(OperationVariant::Binary(BinaryOperation {
                variant: BinaryOperationVariant::LessThan as i32,
                left: less_than.left.into_proto(),
                right: less_than.right.into_proto(),
            })),
        }
    }

    pub(crate) fn less_or_equal_than(less_or_equal_than: LessOrEqualThan) -> Operation {
        Operation {
            id: less_or_equal_than.id.into_proto(),
            r#type: Some(less_or_equal_than.ty.into_proto()),
            source_ref_index: less_or_equal_than.source_ref_index.into_proto(),
            operation: Some(OperationVariant::Binary(BinaryOperation {
                variant: BinaryOperationVariant::LessEq as i32,
                left: less_or_equal_than.left.into_proto(),
                right: less_or_equal_than.right.into_proto(),
            })),
        }
    }

    pub(crate) fn greater_than(greater_than: GreaterThan) -> Operation {
        Operation {
            id: greater_than.id.into_proto(),
            r#type: Some(greater_than.ty.into_proto()),
            source_ref_index: greater_than.source_ref_index.into_proto(),
            operation: Some(OperationVariant::Binary(BinaryOperation {
                variant: BinaryOperationVariant::GreaterThan as i32,
                left: greater_than.left.into_proto(),
                right: greater_than.right.into_proto(),
            })),
        }
    }

    pub(crate) fn greater_or_equal_than(greater_or_equal_than: GreaterOrEqualThan) -> Operation {
        Operation {
            id: greater_or_equal_than.id.into_proto(),
            r#type: Some(greater_or_equal_than.ty.into_proto()),
            source_ref_index: greater_or_equal_than.source_ref_index.into_proto(),
            operation: Some(OperationVariant::Binary(BinaryOperation {
                variant: BinaryOperationVariant::GreaterEq as i32,
                left: greater_or_equal_than.left.into_proto(),
                right: greater_or_equal_than.right.into_proto(),
            })),
        }
    }

    pub(crate) fn public_output_equality(public_output_equality: PublicOutputEquality) -> Operation {
        Operation {
            id: public_output_equality.id.into_proto(),
            r#type: Some(public_output_equality.ty.into_proto()),
            source_ref_index: public_output_equality.source_ref_index.into_proto(),
            operation: Some(OperationVariant::Binary(BinaryOperation {
                variant: BinaryOperationVariant::EqualsPublicOutput as i32,
                left: public_output_equality.left.into_proto(),
                right: public_output_equality.right.into_proto(),
            })),
        }
    }

    pub(crate) fn equals(equals: Equals) -> Operation {
        Operation {
            id: equals.id.into_proto(),
            r#type: Some(equals.ty.into_proto()),
            source_ref_index: equals.source_ref_index.into_proto(),
            operation: Some(OperationVariant::Binary(BinaryOperation {
                variant: BinaryOperationVariant::Equals as i32,
                left: equals.left.into_proto(),
                right: equals.right.into_proto(),
            })),
        }
    }

    pub(crate) fn cast(cast: Cast) -> Operation {
        Operation {
            id: cast.id.into_proto(),
            r#type: Some(cast.ty.clone().into_proto()),
            source_ref_index: cast.source_ref_index.into_proto(),
            operation: Some(OperationVariant::Cast(CastOperation {
                target: cast.target.into_proto(),
                cast_to: Some(cast.ty.into_proto()),
            })),
        }
    }

    pub(crate) fn input_reference(input_reference: InputReference) -> Operation {
        Operation {
            id: input_reference.id.into_proto(),
            r#type: Some(input_reference.ty.into_proto()),
            source_ref_index: input_reference.source_ref_index.into_proto(),
            operation: Some(OperationVariant::InputRef(InputReferenceOperation {
                refers_to: input_reference.refers_to.into_proto(),
            })),
        }
    }

    pub(crate) fn literal_reference(literal_reference: LiteralReference) -> Operation {
        Operation {
            id: literal_reference.id.into_proto(),
            r#type: Some(literal_reference.ty.into_proto()),
            source_ref_index: literal_reference.source_ref_index.into_proto(),
            operation: Some(OperationVariant::LiteralRef(LiteralReferenceOperation {
                refers_to: literal_reference.refers_to.into_proto(),
            })),
        }
    }

    pub(crate) fn nada_function_arg_ref(arg_ref: NadaFunctionArgRef) -> Operation {
        Operation {
            id: arg_ref.id.into_proto(),
            r#type: Some(arg_ref.ty.into_proto()),
            source_ref_index: arg_ref.source_ref_index.into_proto(),
            operation: Some(OperationVariant::ArgRef(NadaFunctionArgRefOperation {
                function_id: arg_ref.function_id.into_proto(),
                refers_to: arg_ref.refers_to.into_proto(),
            })),
        }
    }

    pub(crate) fn modulo(modulo: Modulo) -> Operation {
        Operation {
            id: modulo.id.into_proto(),
            r#type: Some(modulo.ty.into_proto()),
            source_ref_index: modulo.source_ref_index.into_proto(),
            operation: Some(OperationVariant::Binary(BinaryOperation {
                variant: BinaryOperationVariant::Modulo as i32,
                left: modulo.left.into_proto(),
                right: modulo.right.into_proto(),
            })),
        }
    }

    pub(crate) fn power(power: Power) -> Operation {
        Operation {
            id: power.id.into_proto(),
            r#type: Some(power.ty.into_proto()),
            source_ref_index: power.source_ref_index.into_proto(),
            operation: Some(OperationVariant::Binary(BinaryOperation {
                variant: BinaryOperationVariant::Power as i32,
                left: power.left.into_proto(),
                right: power.right.into_proto(),
            })),
        }
    }

    pub(crate) fn division(division: Division) -> Operation {
        Operation {
            id: division.id.into_proto(),
            r#type: Some(division.ty.into_proto()),
            source_ref_index: division.source_ref_index.into_proto(),
            operation: Some(OperationVariant::Binary(BinaryOperation {
                variant: BinaryOperationVariant::Division as i32,
                left: division.left.into_proto(),
                right: division.right.into_proto(),
            })),
        }
    }

    pub(crate) fn nada_function_call(_: NadaFunctionCall) -> Operation {
        panic!("This doesn't exist anymore in the MIR model")
    }

    pub(crate) fn array_accessor(array_accessor: ArrayAccessor) -> Operation {
        Operation {
            id: array_accessor.id.into_proto(),
            r#type: Some(array_accessor.ty.into_proto()),
            source_ref_index: array_accessor.source_ref_index.into_proto(),
            operation: Some(OperationVariant::ArrayAccessor(ArrayAccessorOperation {
                source: array_accessor.source.into_proto(),
                index: array_accessor.index as u32,
            })),
        }
    }

    pub(crate) fn tuple_accessor(tuple_accessor: TupleAccessor) -> Operation {
        Operation {
            id: tuple_accessor.id.into_proto(),
            r#type: Some(tuple_accessor.ty.into_proto()),
            source_ref_index: tuple_accessor.source_ref_index.into_proto(),
            operation: Some(OperationVariant::TupleAccessor(TupleAccessorOperation {
                source: tuple_accessor.source.into_proto(),
                index: tuple_accessor.index.into_proto(),
            })),
        }
    }

    pub(crate) fn new(new: New) -> Operation {
        Operation {
            id: new.id.into_proto(),
            r#type: Some(new.ty.into_proto()),
            source_ref_index: new.source_ref_index.into_proto(),
            operation: Some(OperationVariant::New(NewOperation { elements: new.elements.into_proto() })),
        }
    }

    pub(crate) fn random(random: Random) -> Operation {
        Operation {
            id: random.id.into_proto(),
            r#type: Some(random.ty.into_proto()),
            source_ref_index: random.source_ref_index.into_proto(),
            operation: Some(OperationVariant::Random(())),
        }
    }

    pub(crate) fn if_else(if_else: IfElse) -> Operation {
        Operation {
            id: if_else.id.into_proto(),
            r#type: Some(if_else.ty.into_proto()),
            source_ref_index: if_else.source_ref_index.into_proto(),
            operation: Some(OperationVariant::Ifelse(IfElseOperation {
                cond: if_else.this.into_proto(),
                first: if_else.arg_0.into_proto(),
                second: if_else.arg_1.into_proto(),
            })),
        }
    }

    pub(crate) fn reveal(reveal: Reveal) -> Operation {
        Operation {
            id: reveal.id.into_proto(),
            r#type: Some(reveal.ty.into_proto()),
            source_ref_index: reveal.source_ref_index.into_proto(),
            operation: Some(OperationVariant::Unary(UnaryOperation {
                variant: UnaryOperationVariant::Reveal as i32,
                this: reveal.this.into_proto(),
            })),
        }
    }

    pub(crate) fn public_key_derive(public_key_derive: PublicKeyDerive) -> Operation {
        Operation {
            id: public_key_derive.id.into_proto(),
            r#type: Some(public_key_derive.ty.into_proto()),
            source_ref_index: public_key_derive.source_ref_index.into_proto(),
            operation: Some(OperationVariant::Unary(UnaryOperation {
                variant: UnaryOperationVariant::PublicKeyDerive as i32,
                this: public_key_derive.this.into_proto(),
            })),
        }
    }

    pub(crate) fn not(not: Not) -> Operation {
        Operation {
            id: not.id.into_proto(),
            r#type: Some(not.ty.into_proto()),
            source_ref_index: not.source_ref_index.into_proto(),
            operation: Some(OperationVariant::Unary(UnaryOperation {
                variant: UnaryOperationVariant::Not as i32,
                this: not.this.into_proto(),
            })),
        }
    }

    pub(crate) fn left_shift(left_shift: LeftShift) -> Operation {
        Operation {
            id: left_shift.id.into_proto(),
            r#type: Some(left_shift.ty.into_proto()),
            source_ref_index: left_shift.source_ref_index.into_proto(),
            operation: Some(OperationVariant::Binary(BinaryOperation {
                variant: BinaryOperationVariant::LeftShift as i32,
                left: left_shift.left.into_proto(),
                right: left_shift.right.into_proto(),
            })),
        }
    }

    pub(crate) fn right_shift(right_shift: RightShift) -> Operation {
        Operation {
            id: right_shift.id.into_proto(),
            r#type: Some(right_shift.ty.into_proto()),
            source_ref_index: right_shift.source_ref_index.into_proto(),
            operation: Some(OperationVariant::Binary(BinaryOperation {
                variant: BinaryOperationVariant::RightShift as i32,
                left: right_shift.left.into_proto(),
                right: right_shift.right.into_proto(),
            })),
        }
    }

    pub(crate) fn trunc_pr(trunc_pr: TruncPr) -> Operation {
        Operation {
            id: trunc_pr.id.into_proto(),
            r#type: Some(trunc_pr.ty.into_proto()),
            source_ref_index: trunc_pr.source_ref_index.into_proto(),
            operation: Some(OperationVariant::Binary(BinaryOperation {
                variant: BinaryOperationVariant::TruncPr as i32,
                left: trunc_pr.left.into_proto(),
                right: trunc_pr.right.into_proto(),
            })),
        }
    }

    pub(crate) fn inner_product(inner_product: InnerProduct) -> Operation {
        Operation {
            id: inner_product.id.into_proto(),
            r#type: Some(inner_product.ty.into_proto()),
            source_ref_index: inner_product.source_ref_index.into_proto(),
            operation: Some(OperationVariant::Binary(BinaryOperation {
                variant: BinaryOperationVariant::InnerProduct as i32,
                left: inner_product.left.into_proto(),
                right: inner_product.right.into_proto(),
            })),
        }
    }

    pub(crate) fn not_equals(not_equals: NotEquals) -> Operation {
        Operation {
            id: not_equals.id.into_proto(),
            r#type: Some(not_equals.ty.into_proto()),
            source_ref_index: not_equals.source_ref_index.into_proto(),
            operation: Some(OperationVariant::Binary(BinaryOperation {
                variant: BinaryOperationVariant::NotEquals as i32,
                left: not_equals.left.into_proto(),
                right: not_equals.right.into_proto(),
            })),
        }
    }

    pub(crate) fn boolean_and(boolean_and: BooleanAnd) -> Operation {
        Operation {
            id: boolean_and.id.into_proto(),
            r#type: Some(boolean_and.ty.into_proto()),
            source_ref_index: boolean_and.source_ref_index.into_proto(),
            operation: Some(OperationVariant::Binary(BinaryOperation {
                variant: BinaryOperationVariant::BoolAnd as i32,
                left: boolean_and.left.into_proto(),
                right: boolean_and.right.into_proto(),
            })),
        }
    }

    pub(crate) fn boolean_or(boolean_or: BooleanOr) -> Operation {
        Operation {
            id: boolean_or.id.into_proto(),
            r#type: Some(boolean_or.ty.into_proto()),
            source_ref_index: boolean_or.source_ref_index.into_proto(),
            operation: Some(OperationVariant::Binary(BinaryOperation {
                variant: BinaryOperationVariant::BoolOr as i32,
                left: boolean_or.left.into_proto(),
                right: boolean_or.right.into_proto(),
            })),
        }
    }

    pub(crate) fn boolean_xor(boolean_xor: BooleanXor) -> Operation {
        Operation {
            id: boolean_xor.id.into_proto(),
            r#type: Some(boolean_xor.ty.into_proto()),
            source_ref_index: boolean_xor.source_ref_index.into_proto(),
            operation: Some(OperationVariant::Binary(BinaryOperation {
                variant: BinaryOperationVariant::BoolXor as i32,
                left: boolean_xor.left.into_proto(),
                right: boolean_xor.right.into_proto(),
            })),
        }
    }

    pub(crate) fn ecdsa_sign(ecdsa_sign: EcdsaSign) -> Operation {
        Operation {
            id: ecdsa_sign.id.into_proto(),
            r#type: Some(ecdsa_sign.ty.into_proto()),
            source_ref_index: ecdsa_sign.source_ref_index.into_proto(),
            operation: Some(OperationVariant::Binary(BinaryOperation {
                variant: BinaryOperationVariant::EcdsaSign as i32,
                left: ecdsa_sign.left.into_proto(),
                right: ecdsa_sign.right.into_proto(),
            })),
        }
    }

    pub(crate) fn eddsa_sign(eddsa_sign: EddsaSign) -> Operation {
        Operation {
            id: eddsa_sign.id.into_proto(),
            r#type: Some(eddsa_sign.ty.into_proto()),
            source_ref_index: eddsa_sign.source_ref_index.into_proto(),
            operation: Some(OperationVariant::Binary(BinaryOperation {
                variant: BinaryOperationVariant::EddsaSign as i32,
                left: eddsa_sign.left.into_proto(),
                right: eddsa_sign.right.into_proto(),
            })),
        }
    }
}

mod op_proto_to_rust {
    use crate::{
        proto::{ProtoError, TryIntoRust},
        Addition, ArrayAccessor, BooleanAnd, BooleanOr, BooleanXor, Cast, Division, EcdsaSign, EddsaSign, Equals,
        GreaterOrEqualThan, GreaterThan, IfElse, InnerProduct, InputReference, LeftShift, LessOrEqualThan, LessThan,
        LiteralReference, Map, Modulo, Multiplication, NadaFunctionArgRef, New, Not, NotEquals, Operation, Power,
        PublicKeyDerive, PublicOutputEquality, Random, Reduce, Reveal, RightShift, Subtraction, TruncPr, TupleAccessor,
        Unzip, Zip,
    };
    use mir_proto::nillion::nada::operations::v1::{
        ArrayAccessor as ArrayAccessorOperation, BinaryOperation, CastOperation, IfElseOperation,
        InputReference as InputReferenceOperation, LiteralReference as LiteralReferenceOperation, MapOperation,
        NadaFunctionArgRef as NadaFunctionArgRefOperation, NewOperation, NtupleAccessor as NtupleAccessorOperation,
        ObjectAccessor as ObjectAccessorOperation, Operation as ProtoOperation, ReduceOperation,
        TupleAccessor as TupleAccessorOperation, UnaryOperation,
    };

    pub(crate) fn addition(operation: ProtoOperation, addition: BinaryOperation) -> Result<Operation, ProtoError> {
        Ok(Operation::Addition(Addition {
            id: operation.id.try_into_rust()?,
            ty: operation.r#type.ok_or(ProtoError("type not set"))?.try_into_rust()?,
            source_ref_index: operation.source_ref_index.try_into_rust()?,
            left: addition.left.try_into_rust()?,
            right: addition.right.try_into_rust()?,
        }))
    }

    pub(crate) fn subtraction(
        operation: ProtoOperation,
        subtraction: BinaryOperation,
    ) -> Result<Operation, ProtoError> {
        Ok(Operation::Subtraction(Subtraction {
            id: operation.id.try_into_rust()?,
            ty: operation.r#type.ok_or(ProtoError("type not set"))?.try_into_rust()?,
            source_ref_index: operation.source_ref_index.try_into_rust()?,
            left: subtraction.left.try_into_rust()?,
            right: subtraction.right.try_into_rust()?,
        }))
    }

    pub(crate) fn multiplication(
        operation: ProtoOperation,
        multiplication: BinaryOperation,
    ) -> Result<Operation, ProtoError> {
        Ok(Operation::Multiplication(Multiplication {
            id: operation.id.try_into_rust()?,
            ty: operation.r#type.ok_or(ProtoError("type not set"))?.try_into_rust()?,
            source_ref_index: operation.source_ref_index.try_into_rust()?,
            left: multiplication.left.try_into_rust()?,
            right: multiplication.right.try_into_rust()?,
        }))
    }

    pub(crate) fn less_than(operation: ProtoOperation, less_than: BinaryOperation) -> Result<Operation, ProtoError> {
        Ok(Operation::LessThan(LessThan {
            id: operation.id.try_into_rust()?,
            ty: operation.r#type.ok_or(ProtoError("type not set"))?.try_into_rust()?,
            source_ref_index: operation.source_ref_index.try_into_rust()?,
            left: less_than.left.try_into_rust()?,
            right: less_than.right.try_into_rust()?,
        }))
    }

    pub(crate) fn less_or_equal_than(
        operation: ProtoOperation,
        less_or_equal_than: BinaryOperation,
    ) -> Result<Operation, ProtoError> {
        Ok(Operation::LessOrEqualThan(LessOrEqualThan {
            id: operation.id.try_into_rust()?,
            ty: operation.r#type.ok_or(ProtoError("type not set"))?.try_into_rust()?,
            source_ref_index: operation.source_ref_index.try_into_rust()?,
            left: less_or_equal_than.left.try_into_rust()?,
            right: less_or_equal_than.right.try_into_rust()?,
        }))
    }

    pub(crate) fn greater_than(
        operation: ProtoOperation,
        greater_than: BinaryOperation,
    ) -> Result<Operation, ProtoError> {
        Ok(Operation::GreaterThan(GreaterThan {
            id: operation.id.try_into_rust()?,
            ty: operation.r#type.ok_or(ProtoError("type not set"))?.try_into_rust()?,
            source_ref_index: operation.source_ref_index.try_into_rust()?,
            left: greater_than.left.try_into_rust()?,
            right: greater_than.right.try_into_rust()?,
        }))
    }

    pub(crate) fn greater_or_equal_than(
        operation: ProtoOperation,
        greater_or_equal_than: BinaryOperation,
    ) -> Result<Operation, ProtoError> {
        Ok(Operation::GreaterOrEqualThan(GreaterOrEqualThan {
            id: operation.id.try_into_rust()?,
            ty: operation.r#type.ok_or(ProtoError("type not set"))?.try_into_rust()?,
            source_ref_index: operation.source_ref_index.try_into_rust()?,
            left: greater_or_equal_than.left.try_into_rust()?,
            right: greater_or_equal_than.right.try_into_rust()?,
        }))
    }

    pub(crate) fn public_output_equality(
        operation: ProtoOperation,
        public_output_equality: BinaryOperation,
    ) -> Result<Operation, ProtoError> {
        Ok(Operation::PublicOutputEquality(PublicOutputEquality {
            id: operation.id.try_into_rust()?,
            ty: operation.r#type.ok_or(ProtoError("type not set"))?.try_into_rust()?,
            source_ref_index: operation.source_ref_index.try_into_rust()?,
            left: public_output_equality.left.try_into_rust()?,
            right: public_output_equality.right.try_into_rust()?,
        }))
    }

    pub(crate) fn equals(operation: ProtoOperation, equals: BinaryOperation) -> Result<Operation, ProtoError> {
        Ok(Operation::Equals(Equals {
            id: operation.id.try_into_rust()?,
            ty: operation.r#type.ok_or(ProtoError("type not set"))?.try_into_rust()?,
            source_ref_index: operation.source_ref_index.try_into_rust()?,
            left: equals.left.try_into_rust()?,
            right: equals.right.try_into_rust()?,
        }))
    }

    pub(crate) fn modulo(operation: ProtoOperation, modulo: BinaryOperation) -> Result<Operation, ProtoError> {
        Ok(Operation::Modulo(Modulo {
            id: operation.id.try_into_rust()?,
            ty: operation.r#type.ok_or(ProtoError("type not set"))?.try_into_rust()?,
            source_ref_index: operation.source_ref_index.try_into_rust()?,
            left: modulo.left.try_into_rust()?,
            right: modulo.right.try_into_rust()?,
        }))
    }

    pub(crate) fn power(operation: ProtoOperation, power: BinaryOperation) -> Result<Operation, ProtoError> {
        Ok(Operation::Power(Power {
            id: operation.id.try_into_rust()?,
            ty: operation.r#type.ok_or(ProtoError("type not set"))?.try_into_rust()?,
            source_ref_index: operation.source_ref_index.try_into_rust()?,
            left: power.left.try_into_rust()?,
            right: power.right.try_into_rust()?,
        }))
    }

    pub(crate) fn division(operation: ProtoOperation, division: BinaryOperation) -> Result<Operation, ProtoError> {
        Ok(Operation::Division(Division {
            id: operation.id.try_into_rust()?,
            ty: operation.r#type.ok_or(ProtoError("type not set"))?.try_into_rust()?,
            source_ref_index: operation.source_ref_index.try_into_rust()?,
            left: division.left.try_into_rust()?,
            right: division.right.try_into_rust()?,
        }))
    }

    pub(crate) fn left_shift(operation: ProtoOperation, left_shift: BinaryOperation) -> Result<Operation, ProtoError> {
        Ok(Operation::LeftShift(LeftShift {
            id: operation.id.try_into_rust()?,
            ty: operation.r#type.ok_or(ProtoError("type not set"))?.try_into_rust()?,
            source_ref_index: operation.source_ref_index.try_into_rust()?,
            left: left_shift.left.try_into_rust()?,
            right: left_shift.right.try_into_rust()?,
        }))
    }

    pub(crate) fn right_shift(
        operation: ProtoOperation,
        right_shift: BinaryOperation,
    ) -> Result<Operation, ProtoError> {
        Ok(Operation::RightShift(RightShift {
            id: operation.id.try_into_rust()?,
            ty: operation.r#type.ok_or(ProtoError("type not set"))?.try_into_rust()?,
            source_ref_index: operation.source_ref_index.try_into_rust()?,
            left: right_shift.left.try_into_rust()?,
            right: right_shift.right.try_into_rust()?,
        }))
    }

    pub(crate) fn trunc_pr(operation: ProtoOperation, trunc_pr: BinaryOperation) -> Result<Operation, ProtoError> {
        Ok(Operation::TruncPr(TruncPr {
            id: operation.id.try_into_rust()?,
            ty: operation.r#type.ok_or(ProtoError("type not set"))?.try_into_rust()?,
            source_ref_index: operation.source_ref_index.try_into_rust()?,
            left: trunc_pr.left.try_into_rust()?,
            right: trunc_pr.right.try_into_rust()?,
        }))
    }

    pub(crate) fn not_equals(operation: ProtoOperation, not_equals: BinaryOperation) -> Result<Operation, ProtoError> {
        Ok(Operation::NotEquals(NotEquals {
            id: operation.id.try_into_rust()?,
            ty: operation.r#type.ok_or(ProtoError("type not set"))?.try_into_rust()?,
            source_ref_index: operation.source_ref_index.try_into_rust()?,
            left: not_equals.left.try_into_rust()?,
            right: not_equals.right.try_into_rust()?,
        }))
    }

    pub(crate) fn boolean_and(
        operation: ProtoOperation,
        boolean_and: BinaryOperation,
    ) -> Result<Operation, ProtoError> {
        Ok(Operation::BooleanAnd(BooleanAnd {
            id: operation.id.try_into_rust()?,
            ty: operation.r#type.ok_or(ProtoError("type not set"))?.try_into_rust()?,
            source_ref_index: operation.source_ref_index.try_into_rust()?,
            left: boolean_and.left.try_into_rust()?,
            right: boolean_and.right.try_into_rust()?,
        }))
    }

    pub(crate) fn boolean_or(operation: ProtoOperation, boolean_or: BinaryOperation) -> Result<Operation, ProtoError> {
        Ok(Operation::BooleanOr(BooleanOr {
            id: operation.id.try_into_rust()?,
            ty: operation.r#type.ok_or(ProtoError("type not set"))?.try_into_rust()?,
            source_ref_index: operation.source_ref_index.try_into_rust()?,
            left: boolean_or.left.try_into_rust()?,
            right: boolean_or.right.try_into_rust()?,
        }))
    }

    pub(crate) fn boolean_xor(
        operation: ProtoOperation,
        boolean_xor: BinaryOperation,
    ) -> Result<Operation, ProtoError> {
        Ok(Operation::BooleanXor(BooleanXor {
            id: operation.id.try_into_rust()?,
            ty: operation.r#type.ok_or(ProtoError("type not set"))?.try_into_rust()?,
            source_ref_index: operation.source_ref_index.try_into_rust()?,
            left: boolean_xor.left.try_into_rust()?,
            right: boolean_xor.right.try_into_rust()?,
        }))
    }

    pub(crate) fn zip(operation: ProtoOperation, zip: BinaryOperation) -> Result<Operation, ProtoError> {
        Ok(Operation::Zip(Zip {
            id: operation.id.try_into_rust()?,
            ty: operation.r#type.ok_or(ProtoError("type not set"))?.try_into_rust()?,
            source_ref_index: operation.source_ref_index.try_into_rust()?,
            left: zip.left.try_into_rust()?,
            right: zip.right.try_into_rust()?,
        }))
    }

    pub(crate) fn inner_product(
        operation: ProtoOperation,
        inner_product: BinaryOperation,
    ) -> Result<Operation, ProtoError> {
        Ok(Operation::InnerProduct(InnerProduct {
            id: operation.id.try_into_rust()?,
            ty: operation.r#type.ok_or(ProtoError("type not set"))?.try_into_rust()?,
            source_ref_index: operation.source_ref_index.try_into_rust()?,
            left: inner_product.left.try_into_rust()?,
            right: inner_product.right.try_into_rust()?,
        }))
    }

    pub(crate) fn ecdsa_sign(operation: ProtoOperation, ecdsa_sign: BinaryOperation) -> Result<Operation, ProtoError> {
        Ok(Operation::EcdsaSign(EcdsaSign {
            id: operation.id.try_into_rust()?,
            ty: operation.r#type.ok_or(ProtoError("type not set"))?.try_into_rust()?,
            source_ref_index: operation.source_ref_index.try_into_rust()?,
            left: ecdsa_sign.left.try_into_rust()?,
            right: ecdsa_sign.right.try_into_rust()?,
        }))
    }

    pub(crate) fn eddsa_sign(operation: ProtoOperation, eddsa_sign: BinaryOperation) -> Result<Operation, ProtoError> {
        Ok(Operation::EddsaSign(EddsaSign {
            id: operation.id.try_into_rust()?,
            ty: operation.r#type.ok_or(ProtoError("type not set"))?.try_into_rust()?,
            source_ref_index: operation.source_ref_index.try_into_rust()?,
            left: eddsa_sign.left.try_into_rust()?,
            right: eddsa_sign.right.try_into_rust()?,
        }))
    }

    pub(crate) fn unzip(operation: ProtoOperation, unzip: UnaryOperation) -> Result<Operation, ProtoError> {
        Ok(Operation::Unzip(Unzip {
            id: operation.id.try_into_rust()?,
            ty: operation.r#type.ok_or(ProtoError("type not set"))?.try_into_rust()?,
            source_ref_index: operation.source_ref_index.try_into_rust()?,
            this: unzip.this.try_into_rust()?,
        }))
    }

    pub(crate) fn reveal(operation: ProtoOperation, reveal: UnaryOperation) -> Result<Operation, ProtoError> {
        Ok(Operation::Reveal(Reveal {
            id: operation.id.try_into_rust()?,
            ty: operation.r#type.ok_or(ProtoError("type not set"))?.try_into_rust()?,
            source_ref_index: operation.source_ref_index.try_into_rust()?,
            this: reveal.this.try_into_rust()?,
        }))
    }

    pub(crate) fn public_key_derive(
        operation: ProtoOperation,
        public_key_derive: UnaryOperation,
    ) -> Result<Operation, ProtoError> {
        Ok(Operation::PublicKeyDerive(PublicKeyDerive {
            id: operation.id.try_into_rust()?,
            ty: operation.r#type.ok_or(ProtoError("type not set"))?.try_into_rust()?,
            source_ref_index: operation.source_ref_index.try_into_rust()?,
            this: public_key_derive.this.try_into_rust()?,
        }))
    }

    pub(crate) fn not(operation: ProtoOperation, not: UnaryOperation) -> Result<Operation, ProtoError> {
        Ok(Operation::Not(Not {
            id: operation.id.try_into_rust()?,
            ty: operation.r#type.ok_or(ProtoError("type not set"))?.try_into_rust()?,
            source_ref_index: operation.source_ref_index.try_into_rust()?,
            this: not.this.try_into_rust()?,
        }))
    }

    pub(crate) fn if_else(operation: ProtoOperation, if_else: IfElseOperation) -> Result<Operation, ProtoError> {
        Ok(Operation::IfElse(IfElse {
            id: operation.id.try_into_rust()?,
            ty: operation.r#type.ok_or(ProtoError("type not set"))?.try_into_rust()?,
            source_ref_index: operation.source_ref_index.try_into_rust()?,
            this: if_else.cond.try_into_rust()?,
            arg_0: if_else.first.try_into_rust()?,
            arg_1: if_else.second.try_into_rust()?,
        }))
    }

    pub(crate) fn random(operation: ProtoOperation) -> Result<Operation, ProtoError> {
        Ok(Operation::Random(Random {
            id: operation.id.try_into_rust()?,
            ty: operation.r#type.ok_or(ProtoError("type not set"))?.try_into_rust()?,
            source_ref_index: operation.source_ref_index.try_into_rust()?,
        }))
    }

    pub(crate) fn input_reference(
        operation: ProtoOperation,
        input_reference: InputReferenceOperation,
    ) -> Result<Operation, ProtoError> {
        Ok(Operation::InputReference(InputReference {
            id: operation.id.try_into_rust()?,
            ty: operation.r#type.ok_or(ProtoError("type not set"))?.try_into_rust()?,
            source_ref_index: operation.source_ref_index.try_into_rust()?,
            refers_to: input_reference.refers_to.try_into_rust()?,
        }))
    }

    pub(crate) fn literal_reference(
        operation: ProtoOperation,
        literal_reference: LiteralReferenceOperation,
    ) -> Result<Operation, ProtoError> {
        Ok(Operation::LiteralReference(LiteralReference {
            id: operation.id.try_into_rust()?,
            ty: operation.r#type.ok_or(ProtoError("type not set"))?.try_into_rust()?,
            source_ref_index: operation.source_ref_index.try_into_rust()?,
            refers_to: literal_reference.refers_to.try_into_rust()?,
        }))
    }

    pub(crate) fn arg_reference(
        operation: ProtoOperation,
        arg_ref: NadaFunctionArgRefOperation,
    ) -> Result<Operation, ProtoError> {
        Ok(Operation::NadaFunctionArgRef(NadaFunctionArgRef {
            id: operation.id.try_into_rust()?,
            ty: operation.r#type.ok_or(ProtoError("type not set"))?.try_into_rust()?,
            source_ref_index: operation.source_ref_index.try_into_rust()?,
            refers_to: arg_ref.refers_to.try_into_rust()?,
            function_id: arg_ref.function_id.try_into_rust()?,
        }))
    }

    pub(crate) fn map(operation: ProtoOperation, map: MapOperation) -> Result<Operation, ProtoError> {
        Ok(Operation::Map(Map {
            id: operation.id.try_into_rust()?,
            ty: operation.r#type.ok_or(ProtoError("type not set"))?.try_into_rust()?,
            source_ref_index: operation.source_ref_index.try_into_rust()?,
            inner: map.child.try_into_rust()?,
            function_id: map.r#fn.try_into_rust()?,
        }))
    }

    pub(crate) fn reduce(operation: ProtoOperation, reduce: ReduceOperation) -> Result<Operation, ProtoError> {
        Ok(Operation::Reduce(Reduce {
            id: operation.id.try_into_rust()?,
            ty: operation.r#type.ok_or(ProtoError("type not set"))?.try_into_rust()?,
            source_ref_index: operation.source_ref_index.try_into_rust()?,
            inner: reduce.child.try_into_rust()?,
            function_id: reduce.r#fn.try_into_rust()?,
            initial: reduce.initial.try_into_rust()?,
        }))
    }

    pub(crate) fn new(operation: ProtoOperation, new: NewOperation) -> Result<Operation, ProtoError> {
        Ok(Operation::New(New {
            id: operation.id.try_into_rust()?,
            ty: operation.r#type.ok_or(ProtoError("type not set"))?.try_into_rust()?,
            source_ref_index: operation.source_ref_index.try_into_rust()?,
            elements: new.elements.try_into_rust()?,
        }))
    }

    pub(crate) fn array_accessor(
        operation: ProtoOperation,
        array_accessor: ArrayAccessorOperation,
    ) -> Result<Operation, ProtoError> {
        Ok(Operation::ArrayAccessor(ArrayAccessor {
            id: operation.id.try_into_rust()?,
            ty: operation.r#type.ok_or(ProtoError("type not set"))?.try_into_rust()?,
            source_ref_index: operation.source_ref_index.try_into_rust()?,
            source: array_accessor.source.try_into_rust()?,
            index: array_accessor.index as usize,
        }))
    }

    pub(crate) fn tuple_accessor(
        operation: ProtoOperation,
        tuple_accessor: TupleAccessorOperation,
    ) -> Result<Operation, ProtoError> {
        Ok(Operation::TupleAccessor(TupleAccessor {
            id: operation.id.try_into_rust()?,
            ty: operation.r#type.ok_or(ProtoError("type not set"))?.try_into_rust()?,
            source_ref_index: operation.source_ref_index.try_into_rust()?,
            source: tuple_accessor.source.try_into_rust()?,
            index: tuple_accessor.index.try_into_rust()?,
        }))
    }

    pub(crate) fn ntuple_accessor(_: ProtoOperation, _: NtupleAccessorOperation) -> Result<Operation, ProtoError> {
        Err(ProtoError("ntuple_accessor is not implemented"))
    }

    pub(crate) fn object_accessor(_: ProtoOperation, _: ObjectAccessorOperation) -> Result<Operation, ProtoError> {
        Err(ProtoError("object_accessor is not implemented"))
    }

    pub(crate) fn cast(operation: ProtoOperation, cast: CastOperation) -> Result<Operation, ProtoError> {
        Ok(Operation::Cast(Cast {
            id: operation.id.try_into_rust()?,
            ty: operation.r#type.clone().ok_or(ProtoError("type not set"))?.try_into_rust()?,
            to: operation.r#type.ok_or(ProtoError("type not set"))?.try_into_rust()?,
            source_ref_index: operation.source_ref_index.try_into_rust()?,
            target: cast.target.try_into_rust()?,
        }))
    }
}

impl ConvertProto for TupleIndex {
    type ProtoType = i32;

    fn into_proto(self) -> Self::ProtoType {
        match self {
            TupleIndex::Left => 0,
            TupleIndex::Right => 1,
        }
    }

    fn try_from_proto(model: Self::ProtoType) -> Result<Self, ProtoError> {
        match model {
            0 => Ok(TupleIndex::Left),
            1 => Ok(TupleIndex::Right),
            _ => Err(ProtoError("invalid tuple index")),
        }
    }
}

impl ConvertProto for SourceRef {
    type ProtoType = proto_mir::SourceRef;

    fn into_proto(self) -> Self::ProtoType {
        proto_mir::SourceRef { file: self.file, lineno: self.lineno, offset: self.offset, length: self.length }
    }

    fn try_from_proto(model: Self::ProtoType) -> Result<Self, ProtoError> {
        Ok(SourceRef { file: model.file, lineno: model.lineno, offset: model.offset, length: model.length })
    }
}

impl ConvertProto for Literal {
    type ProtoType = proto_mir::Literal;

    fn into_proto(self) -> Self::ProtoType {
        proto_mir::Literal { name: self.name, value: self.value, r#type: Some(self.ty.into_proto()) }
    }

    fn try_from_proto(model: Self::ProtoType) -> Result<Self, ProtoError> {
        Ok(Literal {
            name: model.name,
            value: model.value,
            ty: model.r#type.ok_or(ProtoError("type not set"))?.try_into_rust()?,
        })
    }
}

impl ConvertProto for OperationId {
    type ProtoType = u64;

    fn into_proto(self) -> Self::ProtoType {
        self.0 as u64
    }

    fn try_from_proto(model: Self::ProtoType) -> Result<Self, ProtoError> {
        Ok(OperationId(model as i64))
    }
}

impl ConvertProto for SourceRefIndex {
    type ProtoType = u64;

    fn into_proto(self) -> Self::ProtoType {
        self.0
    }

    fn try_from_proto(model: Self::ProtoType) -> Result<Self, ProtoError> {
        Ok(SourceRefIndex(model))
    }
}

impl ConvertProto for NadaType {
    type ProtoType = proto_ty::NadaType;

    fn into_proto(self) -> Self::ProtoType {
        use proto_ty::{nada_type::NadaType as ProtoNadaType, Array};
        let nada_type = match self {
            NadaType::Integer => ProtoNadaType::Integer(()),
            NadaType::UnsignedInteger => ProtoNadaType::UnsignedInteger(()),
            NadaType::Boolean => ProtoNadaType::Boolean(()),
            NadaType::SecretInteger => ProtoNadaType::SecretInteger(()),
            NadaType::SecretUnsignedInteger => ProtoNadaType::SecretUnsignedInteger(()),
            NadaType::SecretBoolean => ProtoNadaType::SecretBoolean(()),
            NadaType::ShamirShareInteger => ProtoNadaType::SecretInteger(()),
            NadaType::ShamirShareUnsignedInteger => ProtoNadaType::SecretUnsignedInteger(()),
            NadaType::ShamirShareBoolean => ProtoNadaType::SecretBoolean(()),
            NadaType::Array { inner_type, size } => ProtoNadaType::Array(Box::new(Array {
                contained_type: Some(inner_type.into_proto()),
                size: size as u32,
            })),
            NadaType::Tuple { left_type, right_type } => ProtoNadaType::Tuple(Box::new(proto_ty::Tuple {
                left: Some(left_type.into_proto()),
                right: Some(right_type.into_proto()),
            })),
            NadaType::EcdsaPrivateKey => ProtoNadaType::EcdsaPrivateKey(()),
            NadaType::EddsaPrivateKey => ProtoNadaType::EddsaPrivateKey(()),
            NadaType::EcdsaPublicKey => ProtoNadaType::EcdsaPublicKey(()),
            NadaType::EddsaPublicKey => ProtoNadaType::EddsaPublicKey(()),
            NadaType::NTuple { types } => ProtoNadaType::Ntuple(proto_ty::Ntuple { fields: types.into_proto() }),
            NadaType::EcdsaDigestMessage => ProtoNadaType::EcdsaDigestMessage(()),
            NadaType::EddsaMessage => ProtoNadaType::EddsaMessage(()),
            NadaType::Object { types } => {
                let fields = types
                    .0
                    .into_iter()
                    .map(|(k, v)| proto_ty::ObjectEntry { name: k, r#type: Some(v.into_proto()) })
                    .collect();
                ProtoNadaType::Object(proto_ty::Object { fields })
            }
            NadaType::EcdsaSignature => ProtoNadaType::EcdsaSignature(()),
            NadaType::EddsaSignature => ProtoNadaType::EddsaSignature(()),
            NadaType::SecretBlob | NadaType::StoreId => {
                unreachable!("SecretBlob, StoreId, EcdsaPublicKey and EddsaPublicKey are not valid types in MIR")
            }
        };

        proto_ty::NadaType { nada_type: Some(nada_type) }
    }

    fn try_from_proto(model: Self::ProtoType) -> Result<Self, ProtoError> {
        use proto_ty::nada_type::NadaType as ProtoNadaType;

        let nada_type = model.nada_type.ok_or(ProtoError("nada type not set"))?;
        let nada_type = match nada_type {
            ProtoNadaType::Integer(_) => NadaType::Integer,
            ProtoNadaType::UnsignedInteger(_) => NadaType::UnsignedInteger,
            ProtoNadaType::Boolean(_) => NadaType::Boolean,
            ProtoNadaType::SecretInteger(_) => NadaType::SecretInteger,
            ProtoNadaType::SecretUnsignedInteger(_) => NadaType::SecretUnsignedInteger,
            ProtoNadaType::SecretBoolean(_) => NadaType::SecretBoolean,
            ProtoNadaType::EcdsaPrivateKey(_) => NadaType::EcdsaPrivateKey,
            ProtoNadaType::EcdsaPublicKey(_) => NadaType::EcdsaPublicKey,
            ProtoNadaType::EddsaPrivateKey(_) => NadaType::EddsaPrivateKey,
            ProtoNadaType::EddsaPublicKey(_) => NadaType::EddsaPublicKey,
            ProtoNadaType::EcdsaDigestMessage(_) => NadaType::EcdsaDigestMessage,
            ProtoNadaType::EcdsaSignature(_) => NadaType::EcdsaSignature,
            ProtoNadaType::EddsaSignature(_) => NadaType::EddsaSignature,
            ProtoNadaType::EddsaMessage(_) => NadaType::EddsaMessage,
            ProtoNadaType::Array(array) => NadaType::Array {
                inner_type: array.contained_type.ok_or(ProtoError("contained type not set"))?.try_into_rust()?,
                size: array.size as usize,
            },
            ProtoNadaType::Tuple(tuple) => NadaType::Tuple {
                left_type: tuple.left.ok_or(ProtoError("left type not set"))?.try_into_rust()?,
                right_type: tuple.right.ok_or(ProtoError("right type not set"))?.try_into_rust()?,
            },
            ProtoNadaType::Ntuple(ntuple) => NadaType::NTuple {
                types: ntuple
                    .fields
                    .into_iter()
                    .map(|ty| ty.try_into_rust())
                    .collect::<Result<Vec<_>, ProtoError>>()?,
            },
            ProtoNadaType::Object(object) => {
                let types = object
                    .fields
                    .into_iter()
                    .map(|entry| {
                        Ok((entry.name.clone(), entry.r#type.ok_or(ProtoError("type not set"))?.try_into_rust()?))
                    })
                    .collect::<Result<IndexMap<String, NadaType>, ProtoError>>()?;
                NadaType::Object { types: HashableIndexMap(types) }
            }
        };
        Ok(nada_type)
    }
}
