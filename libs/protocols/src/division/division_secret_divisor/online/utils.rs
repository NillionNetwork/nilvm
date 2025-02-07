//! Utils for division.

use super::super::offline::PrepDivisionIntegerSecretShares;
use crate::{
    bit_operations::bit_decompose::BitDecomposeOperands, multiplication::multiplication_shares::OperandShares,
};
use math_lib::modular::{Modular, ModularNumber};

/// Calculate resulting signs.
pub fn calculate_signs<T: Modular>(
    divisor_signs: Vec<ModularNumber<T>>,
    dividend_signs: Vec<ModularNumber<T>>,
    sign_products: Vec<ModularNumber<T>>,
) -> Vec<ModularNumber<T>> {
    divisor_signs
        .iter()
        .zip(dividend_signs.iter())
        .zip(sign_products.iter())
        .map(|((divisor_sign, dividend_sign), sign_product)| {
            ModularNumber::ONE - divisor_sign - dividend_sign + &(ModularNumber::two() * sign_product)
        })
        .collect()
}

/// Build scale operands from divisors.
pub fn build_scale_operands<T: Modular>(
    abs_divisors: &[ModularNumber<T>],
    prep_elements: &[PrepDivisionIntegerSecretShares<T>],
) -> Vec<BitDecomposeOperands<T>> {
    abs_divisors
        .iter()
        .zip(prep_elements.iter())
        .map(|(divisor, prep_element)| BitDecomposeOperands::new(*divisor, prep_element.prep_bit_decompose.clone()))
        .collect()
}

/// Build scale mult operands.
pub fn build_scale_mult_operands<T: Modular>(
    scales: Vec<ModularNumber<T>>,
    divisors: &[ModularNumber<T>],
    dividends: &[ModularNumber<T>],
) -> Vec<OperandShares<T>> {
    let mut operands = Vec::with_capacity(2 * scales.len());
    for (scale, divisor) in scales.iter().zip(divisors.iter()) {
        let operand = OperandShares::single(*scale, *divisor);
        operands.push(operand);
    }
    for (scale, dividend) in scales.iter().zip(dividends.iter()) {
        let operand = OperandShares::single(*scale, *dividend);
        operands.push(operand);
    }
    operands
}
