use proc_macro::TokenStream;
use proc_macro2::Literal;
use quote::quote;
use syn::{parse_macro_input, Data, DeriveInput};

use crate::helpers::get_variant_attribute;

/// Generates a trait that contains every primitive enum variant as an associated type.
/// Use the `primitive` attribute to mark a variant as a primitive.
pub(crate) fn generate_enum_primitive_to_trait_impl(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let enum_name = input.ident;
    let Data::Enum(data_enum) = input.data else {
        panic!("{} is not an enum", enum_name);
    };

    let trait_items = data_enum.variants.iter().map(|variant| {
        let variant_name = &variant.ident;
        let comment = Literal::string(&format!("/// Underlying type for {}.", variant_name));
        let is_primitive = get_variant_attribute(variant, "primitive").is_some();

        if is_primitive {
            quote! {
                #[doc = #comment]
                type #variant_name: Clone;
            }
        } else {
            quote! {}
        }
    });

    let expanded = quote! {
        /// Primitive types trait. Can be implemented to provide underlying types for NadaValues.
        /// A primitive trait cannot contain other types, contrary to compound types.
        pub trait PrimitiveTypes {
            #(#trait_items)*
        }
    };

    expanded.into()
}
