use nada_compiler_backend::mir::{
    Addition as MIRAddition, Division as MIRDivision, EcdsaSign as MIREcdsaSign, EddsaSign as MIREddsaSign,
    Equals as MIREquals, IfElse as MIRIfElse, InnerProduct as MIRInnerProduct, LeftShift as MIRLeftShift,
    LessThan as MIRLessThan, LiteralReference as MIRLiteralReference, Modulo as MIRModulo,
    Multiplication as MIRMultiplication, New as MIRNew, Not as MIRNot, Power as MIRPower,
    PublicKeyDerive as MIRPublicKeyDerive, PublicOutputEquality as MIRPublicOutputEquality, Random as MIRRandom,
    Reveal as MIRReveal, RightShift as MIRRightShift, Subtraction as MIRSubtraction, TruncPr as MIRTruncPr,
};

use crate::{
    mir2bytecode::{errors::MIR2BytecodeError, MIR2BytecodeContext, TransformOperationResult},
    models::bytecode::{
        memory::BytecodeAddress, Addition, Division, EcdsaSign, EddsaSign, Equals, Get, IfElse, InnerProduct,
        LeftShift, LessThan, LiteralRef, Modulo, Multiplication, New, Not, Power, PublicKeyDerive,
        PublicOutputEquality, Random, Reveal, RightShift, Subtraction, TruncPr,
    },
};

impl LiteralRef {
    pub(crate) fn from_mir(
        context: &MIR2BytecodeContext,
        mir_literal: &MIRLiteralReference,
    ) -> Result<TransformOperationResult, MIR2BytecodeError> {
        let literal_id = context.literal_address(&mir_literal.refers_to)?;
        let operation = LiteralRef {
            literal_id,
            address: BytecodeAddress::default(),
            ty: mir_literal.ty.clone(),
            source_ref_index: (&mir_literal.source_ref_index).into(),
        }
        .into();
        Ok(TransformOperationResult::Operations(vec![operation]))
    }
}

/// This macro generates the transformation of an unary operation.
/// It receives the MIR2BytecodeContext and the MIROperation and returns the BytecodeOperation.
macro_rules! mir2bytecode_unary_operation {
    ($mir_op:ident, $bytecode_op:ident, $mir_operand:ident) => {
        impl $bytecode_op {
            pub(crate) fn from_mir(
                context: &MIR2BytecodeContext,
                mir_operation: &$mir_op,
            ) -> Result<TransformOperationResult, MIR2BytecodeError> {
                // Get the operand address from the original MIR operation ID.
                // It must already exist because the plan has sorted the instructions early.
                let operand = context.operation_address(mir_operation.$mir_operand).map_err(|e| {
                    MIR2BytecodeError::BytecodeElementNotCreated(stringify!($bytecode_op).to_string(), e.to_string())
                })?;
                // Creates the unary operation
                let operations = vec![
                    $bytecode_op {
                        // We assign a dummy memory address, it will be assigned later
                        address: BytecodeAddress::default(),
                        // The operand is the address that we retrived before.
                        operand,
                        // The type is the same that the MIROperation
                        ty: mir_operation.ty.clone(),
                        source_ref_index: (&mir_operation.source_ref_index).into(),
                    }
                    .into(),
                ];
                Ok(TransformOperationResult::Operations(operations))
            }
        }
    };
}

mir2bytecode_unary_operation!(MIRNot, Not, this);
mir2bytecode_unary_operation!(MIRReveal, Reveal, this);
mir2bytecode_unary_operation!(MIRPublicKeyDerive, PublicKeyDerive, this);

/// This macro generates the transformation for a binary operation.
/// It receives the MIR2BytecodeContext and the MIROperation and returns the BytecodeOperation.
macro_rules! mir2bytecode_binary_operation {
    ($mir_op:ident, $bytecode_op:ident) => {
        impl $bytecode_op {
            pub(crate) fn from_mir(
                context: &MIR2BytecodeContext,
                mir_operation: &$mir_op,
            ) -> Result<TransformOperationResult, MIR2BytecodeError> {
                // Get the operand addresses from the original MIR operations ID.
                // They must already exist because the plan has sorted the instructions early.
                let left = context.operation_address(mir_operation.left).map_err(|e| {
                    MIR2BytecodeError::BytecodeElementNotCreated(stringify!($bytecode_op).to_string(), e.to_string())
                })?;
                let right = context.operation_address(mir_operation.right).map_err(|e| {
                    MIR2BytecodeError::BytecodeElementNotCreated(stringify!($bytecode_op).to_string(), e.to_string())
                })?;
                let operations = vec![
                    $bytecode_op {
                        // We assign a dummy memory address, it will be assigned later
                        address: BytecodeAddress::default(),
                        // The right is the address that we retrived before.
                        right,
                        // The left is the address that we retrived before.
                        left,
                        // The type is the same that the MIROperation
                        ty: mir_operation.ty.clone(),
                        source_ref_index: (&mir_operation.source_ref_index).into(),
                    }
                    .into(),
                ];
                Ok(TransformOperationResult::Operations(operations))
            }
        }
    };
}

mir2bytecode_binary_operation!(MIRAddition, Addition);
mir2bytecode_binary_operation!(MIRSubtraction, Subtraction);
mir2bytecode_binary_operation!(MIRMultiplication, Multiplication);
mir2bytecode_binary_operation!(MIRModulo, Modulo);
mir2bytecode_binary_operation!(MIRPower, Power);
mir2bytecode_binary_operation!(MIRLeftShift, LeftShift);
mir2bytecode_binary_operation!(MIRRightShift, RightShift);
mir2bytecode_binary_operation!(MIRTruncPr, TruncPr);
mir2bytecode_binary_operation!(MIRDivision, Division);
mir2bytecode_binary_operation!(MIREquals, Equals);
mir2bytecode_binary_operation!(MIRLessThan, LessThan);
mir2bytecode_binary_operation!(MIRPublicOutputEquality, PublicOutputEquality);
mir2bytecode_binary_operation!(MIRInnerProduct, InnerProduct);
mir2bytecode_binary_operation!(MIREcdsaSign, EcdsaSign);
mir2bytecode_binary_operation!(MIREddsaSign, EddsaSign);

impl IfElse {
    pub(crate) fn from_mir(
        context: &MIR2BytecodeContext,
        mir_operation: &MIRIfElse,
    ) -> Result<TransformOperationResult, MIR2BytecodeError> {
        // Get the operand addresses from the original MIR operations ID.
        // They must already exist because the plan has sorted the instructions early.
        let first = context.operation_address(mir_operation.this).map_err(|e| {
            MIR2BytecodeError::BytecodeElementNotCreated(stringify!($bytecode_op).to_string(), e.to_string())
        })?;
        let second = context.operation_address(mir_operation.arg_0).map_err(|e| {
            MIR2BytecodeError::BytecodeElementNotCreated(stringify!($bytecode_op).to_string(), e.to_string())
        })?;
        let third = context.operation_address(mir_operation.arg_1).map_err(|e| {
            MIR2BytecodeError::BytecodeElementNotCreated(stringify!($bytecode_op).to_string(), e.to_string())
        })?;

        let if_else = IfElse {
            // We assign a dummy memory address, it will be assigned later
            address: BytecodeAddress::default(),
            // First is the address that we retrived before.
            first,
            // Second is the address that we retrived before.
            second,
            // Third is the address that we retrived before.
            third,
            // The type is the same that the MIROperation
            ty: mir_operation.ty.clone(),
            source_ref_index: (&mir_operation.source_ref_index).into(),
        }
        .into();
        Ok(TransformOperationResult::Operations(vec![if_else]))
    }
}

impl New {
    /// Transforms a MIR New into its bytecode representation.
    ///
    /// A MIR New contains a number of elements, each one of these can be a MIR Operation tree.
    pub(crate) fn from_mir(
        context: &MIR2BytecodeContext,
        mir_new: &MIRNew,
    ) -> Result<TransformOperationResult, MIR2BytecodeError> {
        let source_ref_index = &mir_new.source_ref_index;
        let new_op = New {
            address: BytecodeAddress::default(),
            ty: mir_new.ty.clone(),
            source_ref_index: source_ref_index.into(),
        };
        let mut operations = vec![new_op.into()];
        for inner_operation_id in mir_new.elements.iter() {
            let source_address = context.operation_address(*inner_operation_id)?;
            let source_ty = context.bytecode.memory_element_type(source_address)?;
            let get_op = Get {
                source_address,
                address: BytecodeAddress::default(),
                ty: source_ty.clone(),
                source_ref_index: source_ref_index.into(),
            };
            operations.push(get_op.into());
        }
        Ok(TransformOperationResult::Operations(operations))
    }
}

impl Random {
    pub(crate) fn from_mir(
        _context: &MIR2BytecodeContext,
        mir_operation: &MIRRandom,
    ) -> Result<TransformOperationResult, MIR2BytecodeError> {
        // Get the operand address from the original MIR operation ID.
        // It must already exist because the plan has sorted the instructions early.

        // Creates the unary operation
        let operations = vec![
            Random {
                // We assign a dummy memory address, it will be assigned later
                address: BytecodeAddress::default(),
                // The type is the same that the MIROperation
                ty: mir_operation.ty.clone(),
                source_ref_index: (&mir_operation.source_ref_index).into(),
            }
            .into(),
        ];
        Ok(TransformOperationResult::Operations(operations))
    }
}
