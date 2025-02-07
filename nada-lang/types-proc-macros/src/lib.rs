//! Macros to help implementing basic types.

// We allow panics here because this code can only by used during compilation.
#![deny(missing_docs)]
#![forbid(unsafe_code)]
#![deny(
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::iterator_step_by_zero,
    clippy::invalid_regex,
    clippy::string_slice
)]

mod helpers;
mod is_primitive;
mod new_functions;
mod primitive_to_trait;
mod to_nada_type;
mod to_nada_type_kind;

use is_primitive::generate_is_primitive_functions_impl;
use primitive_to_trait::generate_enum_primitive_to_trait_impl;
use proc_macro::TokenStream;
use to_nada_type::generate_to_nada_type_impl;
use to_nada_type_kind::generate_to_nada_type_kind_impl;

use crate::new_functions::generate_enum_new_functions_impl;

/// Generates a trait that contains every primitive enum variant as an associated type.
/// Use the `primitive` attribute to mark a variant as a primitive.
#[proc_macro_derive(EnumPrimitiveToTrait, attributes(primitive))]
pub fn generate_enum_primitive_to_trait(input: TokenStream) -> TokenStream {
    generate_enum_primitive_to_trait_impl(input)
}

/// Generates `is_primitive` functions for an enum.
/// Use the `primitive` attribute to mark a variant as a primitive.
#[proc_macro_derive(EnumIsPrimitive, attributes(primitive))]
pub fn generate_is_primitive_functions(input: TokenStream) -> TokenStream {
    generate_is_primitive_functions_impl(input)
}

/// Generates `to_nada_type` and `into_nada_type` functions for an enum.
#[proc_macro_derive(EnumToNadaTypeKind)]
pub fn generate_to_nada_type_kind(input: TokenStream) -> TokenStream {
    generate_to_nada_type_kind_impl(input)
}

/// Generates `to_type` and `into_type` functions for an enum.
/// Use `to_type_functions(to_type = my_variant_to_type, into_type = my_variant_into_type)` to specify a function that
/// should be called instead of relying on the automatically generated one.
#[proc_macro_derive(EnumToNadaType, attributes(to_type_functions))]
pub fn generate_to_nada_type(input: TokenStream) -> TokenStream {
    generate_to_nada_type_impl(input)
}

/// Generates a new_* function for each enum variant.
/// Use the `skip_new_function` attribute to skip new function generation.
#[proc_macro_derive(EnumNewFunctions, attributes(skip_new_function))]
pub fn generate_enum_new_functions(input: TokenStream) -> TokenStream {
    generate_enum_new_functions_impl(input)
}
