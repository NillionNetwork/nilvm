use crate::{
    EvaluationError,
    EvaluationError::{InvalidOperandTypes, MismatchedTypes},
};
use math_lib::modular::{FloorMod, ModularNumber, ModularPow, Prime};
use nada_value::{
    clear_modular::ClearModular, errors::ClearModularError, NadaPrimitiveType, NadaType, NadaTypeMetadata, NadaValue,
    Shape,
};
use num_bigint::BigInt;
use std::mem::discriminant;

pub(crate) struct OperationDisplay {
    pub(crate) name: &'static str,
    pub(crate) symbol: &'static str,
}

pub(crate) trait UnaryOperation {
    fn display_info(&self) -> OperationDisplay;
    fn output_type<T: Prime>(&self, operand: &NadaValue<ClearModular<T>>) -> Result<NadaType, EvaluationError> {
        Ok(operand.to_type())
    }

    fn execute<T: Prime>(&self, operand: NadaValue<ClearModular<T>>) -> Result<ModularNumber<T>, EvaluationError>;
}

pub(crate) struct NotOperation;

impl UnaryOperation for NotOperation {
    fn display_info(&self) -> OperationDisplay {
        OperationDisplay { name: "not", symbol: "!" }
    }

    fn execute<T: Prime>(&self, operand: NadaValue<ClearModular<T>>) -> Result<ModularNumber<T>, EvaluationError> {
        let value = ModularNumber::ONE - &operand.try_into()?;
        Ok(value)
    }
}

pub(crate) struct RevealOperation;

impl UnaryOperation for RevealOperation {
    fn display_info(&self) -> OperationDisplay {
        OperationDisplay { name: "reveal", symbol: "reveal" }
    }

    fn output_type<T: Prime>(&self, operand: &NadaValue<ClearModular<T>>) -> Result<NadaType, EvaluationError> {
        let operand_type: NadaTypeMetadata = (&operand.to_type()).into();
        let output_primitive_type = if let Some(primitive_type) = operand_type.nada_primitive_type() {
            primitive_type
        } else {
            return Err(InvalidOperandTypes);
        };

        let output_type = NadaTypeMetadata::PrimitiveType {
            nada_primitive_type: output_primitive_type,
            shape: Shape::PublicVariable,
        };
        Ok((&output_type).try_into()?)
    }

    fn execute<T: Prime>(&self, operand: NadaValue<ClearModular<T>>) -> Result<ModularNumber<T>, EvaluationError> {
        Ok(operand.try_into()?)
    }
}

pub(crate) struct PublicKeyDeriveOperation;

impl UnaryOperation for PublicKeyDeriveOperation {
    fn display_info(&self) -> OperationDisplay {
        OperationDisplay { name: "public_key_derive", symbol: "public_key_derive" }
    }

    fn output_type<T: Prime>(&self, operand: &NadaValue<ClearModular<T>>) -> Result<NadaType, EvaluationError> {
        let operand_type: NadaTypeMetadata = (&operand.to_type()).into();
        let output_primitive_type = if let Some(primitive_type) = operand_type.nada_primitive_type() {
            primitive_type
        } else {
            return Err(InvalidOperandTypes);
        };

        let output_type = NadaTypeMetadata::PrimitiveType {
            nada_primitive_type: output_primitive_type,
            shape: Shape::PublicVariable,
        };
        Ok((&output_type).try_into()?)
    }

    fn execute<T: Prime>(&self, operand: NadaValue<ClearModular<T>>) -> Result<ModularNumber<T>, EvaluationError> {
        Ok(operand.try_into()?)
    }
}

fn default_arithmetic_operation_output_type<T: Prime>(
    lhs: &NadaValue<ClearModular<T>>,
    rhs: &NadaValue<ClearModular<T>>,
) -> Result<NadaType, EvaluationError> {
    let lhs_type: NadaTypeMetadata = (&lhs.to_type()).into();
    let rhs_type: NadaTypeMetadata = (&rhs.to_type()).into();

    let output_primitive_type = match (lhs_type.nada_primitive_type(), rhs_type.nada_primitive_type()) {
        (Some(lhs_primitive_type), Some(rhs_primitive_type)) => {
            if discriminant(&lhs_primitive_type) != discriminant(&rhs_primitive_type) {
                return Err(MismatchedTypes);
            }
            lhs_primitive_type
        }
        (_, _) => return Err(InvalidOperandTypes),
    };

    // Unwraps in this point shouldn't fail. If the operand are compound types, they failed in the previous
    // match, when we evaluate the primitive type.
    let output_shape = if lhs_type.is_private().unwrap() || rhs_type.is_private().unwrap() {
        Shape::Secret
    } else {
        Shape::PublicVariable
    };

    let output_type =
        NadaTypeMetadata::PrimitiveType { nada_primitive_type: output_primitive_type, shape: output_shape };
    Ok((&output_type).try_into()?)
}

fn default_relational_operation_output_type<T: Prime>(
    lhs: &NadaValue<ClearModular<T>>,
    rhs: &NadaValue<ClearModular<T>>,
) -> Result<NadaType, EvaluationError> {
    let lhs_type: NadaTypeMetadata = (&lhs.to_type()).into();
    let rhs_type: NadaTypeMetadata = (&rhs.to_type()).into();

    match (lhs_type.nada_primitive_type(), rhs_type.nada_primitive_type()) {
        (Some(lhs_primitive_type), Some(rhs_primitive_type)) => {
            if discriminant(&lhs_primitive_type) != discriminant(&rhs_primitive_type) {
                return Err(MismatchedTypes);
            }
        }
        (_, _) => return Err(InvalidOperandTypes),
    };

    // Unwraps in this point shouldn't fail. If the operand are compound types, they failed in the previous
    // match, when we evaluate the primitive type.
    let output_shape = if lhs_type.is_private().unwrap() || rhs_type.is_private().unwrap() {
        Shape::Secret
    } else {
        Shape::PublicVariable
    };

    let output_type =
        NadaTypeMetadata::PrimitiveType { nada_primitive_type: NadaPrimitiveType::Boolean, shape: output_shape };
    Ok((&output_type).try_into()?)
}

fn if_else_operation_output_type<T: Prime>(
    cond: &NadaValue<ClearModular<T>>,
    left: &NadaValue<ClearModular<T>>,
    right: &NadaValue<ClearModular<T>>,
) -> Result<NadaType, EvaluationError> {
    let cond_type = &cond.to_type();
    let left_type = &left.to_type();
    let right_type = &right.to_type();

    // Here we check that the underlying types match.
    let output_primitive_type = match (
        Into::<NadaTypeMetadata>::into(left_type).nada_primitive_type(),
        Into::<NadaTypeMetadata>::into(right_type).nada_primitive_type(),
    ) {
        (Some(lhs_primitive_type), Some(rhs_primitive_type)) => {
            if discriminant(&lhs_primitive_type) != discriminant(&rhs_primitive_type) {
                return Err(MismatchedTypes);
            }
            lhs_primitive_type
        }
        (_, _) => return Err(InvalidOperandTypes),
    };

    // If the condition is public, then:
    // * if the two operands are public, the result is public.
    // * if at least one of the two operands is secret, the result is secret.
    //
    // If the condition is secret, then:
    // * the result is always secret.
    let output_shape = if cond_type.is_public() {
        if left_type.is_public() && right_type.is_public() {
            Shape::PublicVariable
        } else if left_type.is_secret() || right_type.is_secret() {
            Shape::Secret
        } else {
            return Err(MismatchedTypes);
        }
    } else {
        Shape::Secret
    };

    let output_type =
        NadaTypeMetadata::PrimitiveType { nada_primitive_type: output_primitive_type, shape: output_shape };
    Ok((&output_type).try_into()?)
}

pub(crate) trait BinaryOperation {
    fn display_info(&self) -> OperationDisplay;

    fn output_type<T: Prime>(
        &self,
        lhs: &NadaValue<ClearModular<T>>,
        rhs: &NadaValue<ClearModular<T>>,
    ) -> Result<NadaType, EvaluationError>;

    fn execute<T: Prime>(
        &self,
        lhs: NadaValue<ClearModular<T>>,
        rhs: NadaValue<ClearModular<T>>,
    ) -> Result<ModularNumber<T>, EvaluationError>;
}

pub(crate) struct AddOperation;

impl BinaryOperation for AddOperation {
    fn display_info(&self) -> OperationDisplay {
        OperationDisplay { name: "addition", symbol: "+" }
    }

    fn output_type<T: Prime>(
        &self,
        lhs: &NadaValue<ClearModular<T>>,
        rhs: &NadaValue<ClearModular<T>>,
    ) -> Result<NadaType, EvaluationError> {
        default_arithmetic_operation_output_type(lhs, rhs)
    }

    fn execute<T: Prime>(
        &self,
        lhs: NadaValue<ClearModular<T>>,
        rhs: NadaValue<ClearModular<T>>,
    ) -> Result<ModularNumber<T>, EvaluationError> {
        let left: ModularNumber<T> = lhs.try_into()?;
        let right: ModularNumber<T> = rhs.try_into()?;
        Ok(left + &right)
    }
}

pub(crate) struct SubOperation;

impl BinaryOperation for SubOperation {
    fn display_info(&self) -> OperationDisplay {
        OperationDisplay { name: "subtraction", symbol: "-" }
    }

    fn output_type<T: Prime>(
        &self,
        lhs: &NadaValue<ClearModular<T>>,
        rhs: &NadaValue<ClearModular<T>>,
    ) -> Result<NadaType, EvaluationError> {
        default_arithmetic_operation_output_type(lhs, rhs)
    }

    fn execute<T: Prime>(
        &self,
        lhs: NadaValue<ClearModular<T>>,
        rhs: NadaValue<ClearModular<T>>,
    ) -> Result<ModularNumber<T>, EvaluationError> {
        let left: ModularNumber<T> = lhs.try_into()?;
        let right: ModularNumber<T> = rhs.try_into()?;
        Ok(left - &right)
    }
}

pub(crate) struct MulOperation;

impl BinaryOperation for MulOperation {
    fn display_info(&self) -> OperationDisplay {
        OperationDisplay { name: "multiplication", symbol: "*" }
    }

    fn output_type<T: Prime>(
        &self,
        lhs: &NadaValue<ClearModular<T>>,
        rhs: &NadaValue<ClearModular<T>>,
    ) -> Result<NadaType, EvaluationError> {
        default_arithmetic_operation_output_type(lhs, rhs)
    }

    fn execute<T: Prime>(
        &self,
        lhs: NadaValue<ClearModular<T>>,
        rhs: NadaValue<ClearModular<T>>,
    ) -> Result<ModularNumber<T>, EvaluationError> {
        let left: ModularNumber<T> = lhs.try_into()?;
        let right: ModularNumber<T> = rhs.try_into()?;
        Ok(left * &right)
    }
}

pub(crate) struct ModuloOperation;

impl BinaryOperation for ModuloOperation {
    fn display_info(&self) -> OperationDisplay {
        OperationDisplay { name: "modulo", symbol: "%" }
    }

    fn output_type<T: Prime>(
        &self,
        lhs: &NadaValue<ClearModular<T>>,
        rhs: &NadaValue<ClearModular<T>>,
    ) -> Result<NadaType, EvaluationError> {
        default_arithmetic_operation_output_type(lhs, rhs)
    }

    fn execute<T: Prime>(
        &self,
        lhs: NadaValue<ClearModular<T>>,
        rhs: NadaValue<ClearModular<T>>,
    ) -> Result<ModularNumber<T>, EvaluationError> {
        let left: ModularNumber<T> = lhs.try_into()?;
        let right: ModularNumber<T> = rhs.try_into()?;
        Ok(left.fmod(&right)?)
    }
}

pub(crate) struct PowerOperation;

impl BinaryOperation for PowerOperation {
    fn display_info(&self) -> OperationDisplay {
        OperationDisplay { name: "power", symbol: "**" }
    }

    fn output_type<T: Prime>(
        &self,
        lhs: &NadaValue<ClearModular<T>>,
        rhs: &NadaValue<ClearModular<T>>,
    ) -> Result<NadaType, EvaluationError> {
        if !rhs.to_type().is_public() {
            return Err(EvaluationError::NotAllowedOperand("secret exponents are not supported for Power"))?;
        }
        default_arithmetic_operation_output_type(lhs, rhs)
    }

    fn execute<T: Prime>(
        &self,
        lhs: NadaValue<ClearModular<T>>,
        rhs: NadaValue<ClearModular<T>>,
    ) -> Result<ModularNumber<T>, EvaluationError> {
        let exponent: ModularNumber<T> = rhs.try_into()?;
        let base: ModularNumber<T> = lhs.try_into()?;
        Ok(base.exp_mod(&exponent.into_value()))
    }
}

pub(crate) struct LeftShiftOperation;

impl BinaryOperation for LeftShiftOperation {
    fn display_info(&self) -> OperationDisplay {
        OperationDisplay { name: "lshift", symbol: "<<" }
    }

    fn output_type<T: Prime>(
        &self,
        lhs: &NadaValue<ClearModular<T>>,
        rhs: &NadaValue<ClearModular<T>>,
    ) -> Result<NadaType, EvaluationError> {
        if !rhs.to_type().is_public() {
            return Err(EvaluationError::NotAllowedOperand("secret shift amount is not supported for LeftShift"))?;
        }
        // Note that there is no need to check that the underlying types match
        // since the shift amount is always an unsigned integer.
        let left_type = &lhs.to_type();
        let output_primitive_type =
            if let Some(primitive_type) = Into::<NadaTypeMetadata>::into(left_type).nada_primitive_type() {
                primitive_type
            } else {
                return Err(InvalidOperandTypes);
            };

        // If the left operand is public, then the result is public.
        let output_shape = if left_type.is_public() { Shape::PublicVariable } else { Shape::Secret };

        let output_type =
            NadaTypeMetadata::PrimitiveType { nada_primitive_type: output_primitive_type, shape: output_shape };
        Ok((&output_type).try_into()?)
    }

    fn execute<T: Prime>(
        &self,
        lhs: NadaValue<ClearModular<T>>,
        rhs: NadaValue<ClearModular<T>>,
    ) -> Result<ModularNumber<T>, EvaluationError> {
        let right_value: ModularNumber<T> = rhs.try_into()?;
        let left_value: ModularNumber<T> = lhs.try_into()?;
        Ok(left_value * &ModularNumber::two().exp_mod(&right_value.into_value()))
    }
}

pub(crate) struct RightShiftOperation;

impl BinaryOperation for RightShiftOperation {
    fn display_info(&self) -> OperationDisplay {
        OperationDisplay { name: "rshift", symbol: ">>" }
    }

    fn output_type<T: Prime>(
        &self,
        lhs: &NadaValue<ClearModular<T>>,
        rhs: &NadaValue<ClearModular<T>>,
    ) -> Result<NadaType, EvaluationError> {
        if !rhs.to_type().is_public() {
            return Err(EvaluationError::NotAllowedOperand("secret shift amount is not supported for RightShift"))?;
        }
        // Note that there is no need to check that the underlying types match
        // since the shift amount is always an unsigned integer.
        let left_type = &lhs.to_type();
        let output_primitive_type =
            if let Some(primitive_type) = Into::<NadaTypeMetadata>::into(left_type).nada_primitive_type() {
                primitive_type
            } else {
                return Err(InvalidOperandTypes);
            };

        // If the left operand is public, then the result is public.
        let output_shape = if left_type.is_public() { Shape::PublicVariable } else { Shape::Secret };

        let output_type =
            NadaTypeMetadata::PrimitiveType { nada_primitive_type: output_primitive_type, shape: output_shape };
        Ok((&output_type).try_into()?)
    }

    fn execute<T: Prime>(
        &self,
        lhs: NadaValue<ClearModular<T>>,
        rhs: NadaValue<ClearModular<T>>,
    ) -> Result<ModularNumber<T>, EvaluationError> {
        let left_value: ModularNumber<T> = lhs.try_into()?;
        let right_value: ModularNumber<T> = rhs.try_into()?;
        let right_value = ModularNumber::two().exp_mod(&right_value.into_value());
        let remainder = left_value.fmod(&right_value)?;
        let dividend = left_value - &remainder;
        Ok((dividend / &right_value)?)
    }
}

pub(crate) struct TruncPrOperation;

impl BinaryOperation for TruncPrOperation {
    fn display_info(&self) -> OperationDisplay {
        OperationDisplay { name: "trunc_pr", symbol: "trunc_pr" }
    }

    fn output_type<T: Prime>(
        &self,
        lhs: &NadaValue<ClearModular<T>>,
        rhs: &NadaValue<ClearModular<T>>,
    ) -> Result<NadaType, EvaluationError> {
        if !rhs.to_type().is_public() {
            return Err(EvaluationError::NotAllowedOperand("secret shift amount is not supported for TruncPr"))?;
        }
        // Note that there is no need to check that the underlying types match
        // since the shift amount is always an (unsigned) integer.
        let left_type = &lhs.to_type();
        let output_primitive_type =
            if let Some(primitive_type) = Into::<NadaTypeMetadata>::into(left_type).nada_primitive_type() {
                primitive_type
            } else {
                return Err(InvalidOperandTypes);
            };

        // If the left operand is public, then the result is public.
        let output_shape = if left_type.is_public() { Shape::PublicVariable } else { Shape::Secret };

        let output_type =
            NadaTypeMetadata::PrimitiveType { nada_primitive_type: output_primitive_type, shape: output_shape };
        Ok((&output_type).try_into()?)
    }

    fn execute<T: Prime>(
        &self,
        lhs: NadaValue<ClearModular<T>>,
        rhs: NadaValue<ClearModular<T>>,
    ) -> Result<ModularNumber<T>, EvaluationError> {
        let left: ModularNumber<T> = lhs.try_into()?;
        let right: ModularNumber<T> = rhs.try_into()?;
        let right_value = ModularNumber::two().exp_mod(&right.into_value());
        let remainder = left.fmod(&right_value)?;
        let dividend = left - &remainder;
        Ok((dividend / &right_value)?)
    }
}

pub(crate) struct LtOperation;

impl BinaryOperation for LtOperation {
    fn display_info(&self) -> OperationDisplay {
        OperationDisplay { name: "less-than", symbol: "<" }
    }

    fn output_type<T: Prime>(
        &self,
        lhs: &NadaValue<ClearModular<T>>,
        rhs: &NadaValue<ClearModular<T>>,
    ) -> Result<NadaType, EvaluationError> {
        default_relational_operation_output_type(lhs, rhs)
    }

    fn execute<T: Prime>(
        &self,
        lhs: NadaValue<ClearModular<T>>,
        rhs: NadaValue<ClearModular<T>>,
    ) -> Result<ModularNumber<T>, EvaluationError> {
        let value = if let NadaValue::Integer(_) | NadaValue::SecretInteger(_) = lhs {
            let left = BigInt::from(&lhs.try_into()?);
            let right = BigInt::from(&rhs.try_into()?);
            left < right
        } else {
            let left: ModularNumber<T> = lhs.try_into()?;
            let right: ModularNumber<T> = rhs.try_into()?;
            left.lt(&right)
        };
        Ok(ModularNumber::from_u32(value as u32))
    }
}

pub(crate) struct DivOperation;

impl BinaryOperation for DivOperation {
    fn display_info(&self) -> OperationDisplay {
        OperationDisplay { name: "division", symbol: "/" }
    }

    fn output_type<T: Prime>(
        &self,
        lhs: &NadaValue<ClearModular<T>>,
        rhs: &NadaValue<ClearModular<T>>,
    ) -> Result<NadaType, EvaluationError> {
        default_arithmetic_operation_output_type(lhs, rhs)
    }

    fn execute<T: Prime>(
        &self,
        lhs: NadaValue<ClearModular<T>>,
        rhs: NadaValue<ClearModular<T>>,
    ) -> Result<ModularNumber<T>, EvaluationError> {
        match lhs {
            NadaValue::Integer(left_value)
            | NadaValue::UnsignedInteger(left_value)
            | NadaValue::SecretInteger(left_value)
            | NadaValue::SecretUnsignedInteger(left_value) => {
                // Integer division of modular numbers gives a 'correct' equivalent value
                // for exact divisions, without remainder
                let right_value = rhs.try_into()?;
                let remainder = left_value.fmod(&right_value)?;
                let dividend = left_value - &remainder;
                Ok((dividend / &right_value)?)
            }
            _ => Err(InvalidOperandTypes),
        }
    }
}

pub(crate) struct PublicOutputEqualityOperation;

impl BinaryOperation for PublicOutputEqualityOperation {
    fn display_info(&self) -> OperationDisplay {
        OperationDisplay { name: "public_output_equality", symbol: "public_output_equality" }
    }

    fn output_type<T: Prime>(
        &self,
        lhs: &NadaValue<ClearModular<T>>,
        rhs: &NadaValue<ClearModular<T>>,
    ) -> Result<NadaType, EvaluationError> {
        let lhs_type: NadaTypeMetadata = (&lhs.to_type()).into();
        let rhs_type: NadaTypeMetadata = (&rhs.to_type()).into();

        match (lhs_type.nada_primitive_type(), rhs_type.nada_primitive_type()) {
            (Some(lhs_primitive_type), Some(rhs_primitive_type)) => {
                if discriminant(&lhs_primitive_type) != discriminant(&rhs_primitive_type) {
                    return Err(MismatchedTypes);
                }
            }
            (_, _) => return Err(InvalidOperandTypes),
        };

        Ok(NadaType::Boolean)
    }

    fn execute<T: Prime>(
        &self,
        lhs: NadaValue<ClearModular<T>>,
        rhs: NadaValue<ClearModular<T>>,
    ) -> Result<ModularNumber<T>, EvaluationError> {
        let left: ModularNumber<T> = lhs.try_into()?;
        let right: ModularNumber<T> = rhs.try_into()?;
        let value = left == right;
        Ok(ModularNumber::from_u32(value as u32))
    }
}

pub(crate) struct EqualsOperation;

impl BinaryOperation for EqualsOperation {
    fn display_info(&self) -> OperationDisplay {
        OperationDisplay { name: "equals_integer_secret", symbol: "equals_integer_secret" }
    }

    fn output_type<T: Prime>(
        &self,
        lhs: &NadaValue<ClearModular<T>>,
        rhs: &NadaValue<ClearModular<T>>,
    ) -> Result<NadaType, EvaluationError> {
        default_relational_operation_output_type(lhs, rhs)
    }

    fn execute<T: Prime>(
        &self,
        lhs: NadaValue<ClearModular<T>>,
        rhs: NadaValue<ClearModular<T>>,
    ) -> Result<ModularNumber<T>, EvaluationError> {
        let left: ModularNumber<T> = lhs.try_into()?;
        let right: ModularNumber<T> = rhs.try_into()?;
        let value = left == right;
        Ok(ModularNumber::from_u32(value as u32))
    }
}

pub(crate) trait TernaryOperation {
    fn display_info(&self) -> OperationDisplay;

    fn output_type<T: Prime>(
        &self,
        first: &NadaValue<ClearModular<T>>,
        second: &NadaValue<ClearModular<T>>,
        third: &NadaValue<ClearModular<T>>,
    ) -> Result<NadaType, EvaluationError>;

    fn execute<T: Prime>(
        &self,
        first: NadaValue<ClearModular<T>>,
        second: NadaValue<ClearModular<T>>,
        third: NadaValue<ClearModular<T>>,
    ) -> Result<ModularNumber<T>, EvaluationError>;
}

pub(crate) struct IfElseOperation;

impl TernaryOperation for IfElseOperation {
    fn display_info(&self) -> OperationDisplay {
        OperationDisplay { name: "if_else", symbol: "?" }
    }

    fn output_type<T: Prime>(
        &self,
        cond: &NadaValue<ClearModular<T>>,
        left: &NadaValue<ClearModular<T>>,
        right: &NadaValue<ClearModular<T>>,
    ) -> Result<NadaType, EvaluationError> {
        if_else_operation_output_type(cond, left, right)
    }

    fn execute<T: Prime>(
        &self,
        cond: NadaValue<ClearModular<T>>,
        left: NadaValue<ClearModular<T>>,
        right: NadaValue<ClearModular<T>>,
    ) -> Result<ModularNumber<T>, EvaluationError> {
        let cond_val: ModularNumber<T> = cond.try_into()?;
        if cond_val != ModularNumber::ZERO { Ok(left.try_into()?) } else { Ok(right.try_into()?) }
    }
}

pub(crate) struct InnerProductOperation;

impl BinaryOperation for InnerProductOperation {
    fn display_info(&self) -> OperationDisplay {
        OperationDisplay { name: "inner_product", symbol: "inner_product" }
    }

    fn output_type<T: Prime>(
        &self,
        lhs: &NadaValue<ClearModular<T>>,
        _: &NadaValue<ClearModular<T>>,
    ) -> Result<NadaType, EvaluationError> {
        if let NadaValue::Array { inner_type, .. } = lhs {
            Ok(inner_type.clone())
        } else {
            Err(EvaluationError::InvalidOperandTypes)
        }
    }

    fn execute<T: Prime>(
        &self,
        lhs: NadaValue<ClearModular<T>>,
        rhs: NadaValue<ClearModular<T>>,
    ) -> Result<ModularNumber<T>, EvaluationError> {
        if let (NadaValue::Array { values: left_values, .. }, NadaValue::Array { values: right_values, .. }) =
            (lhs, rhs)
        {
            let array_of_products = left_values
                .into_iter()
                .zip(right_values)
                .map(|(left, right)| (left * right))
                .collect::<Result<Vec<NadaValue<ClearModular<T>>>, ClearModularError>>()?;
            let mut accummulator = ModularNumber::ZERO;
            for product in array_of_products {
                let product_value = ModularNumber::try_from(product)?;
                accummulator = accummulator + &product_value;
            }
            Ok(accummulator)
        } else {
            Err(EvaluationError::InvalidOperandTypes)
        }
    }
}

#[cfg(test)]
mod tests {
    use math_lib::modular::{ModularNumber, U128SafePrime};
    use nada_value::{clear_modular::ClearModular, NadaType, NadaValue};

    use super::{default_relational_operation_output_type, BinaryOperation, InnerProductOperation};

    #[test]
    fn test_less_than_secret_literal_output_type() {
        let lhs = NadaValue::new_secret_unsigned_integer(ModularNumber::ONE);
        let rhs = NadaValue::new_secret_unsigned_integer(ModularNumber::two());
        let output_type = default_relational_operation_output_type::<U128SafePrime>(&lhs, &rhs).unwrap();
        assert!(output_type.is_secret())
    }

    #[test]
    fn test_inner_product() {
        let lhs: NadaValue<ClearModular<U128SafePrime>> = NadaValue::new_array(
            NadaType::SecretInteger,
            vec![
                NadaValue::new_secret_integer(ModularNumber::from_u32(1)),
                NadaValue::new_secret_integer(ModularNumber::from_u32(2)),
            ],
        )
        .unwrap();
        let rhs = NadaValue::new_array(
            NadaType::SecretInteger,
            vec![
                NadaValue::new_secret_integer(ModularNumber::from_u32(2)),
                NadaValue::new_secret_integer(ModularNumber::from_u32(4)),
            ],
        )
        .unwrap();
        let operation = InnerProductOperation {};
        let output = operation.execute(lhs, rhs).unwrap();
        assert_eq!(ModularNumber::from_u32(10), output);
    }
}
