use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, Data, DeriveInput, Fields};

use crate::helpers::get_variant_attribute;

/// Generates `is_primitive` functions for an enum.
/// Use the `primitive` attribute to mark a variant as a primitive.
pub(crate) fn generate_is_primitive_functions_impl(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let enum_name = input.ident;
    let Data::Enum(data_enum) = input.data else {
        panic!("{} is not an enum", enum_name);
    };
    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();

    let is_primitive_items = data_enum.variants.iter().map(|variant| {
        let variant_name = &variant.ident;
        let is_primitive = get_variant_attribute(variant, "primitive").is_some();

        match &variant.fields {
            Fields::Unit => {
                quote! {
                    #enum_name::#variant_name => #is_primitive,
                }
            }
            Fields::Unnamed(_) => {
                quote! {
                    #enum_name::#variant_name(..) => #is_primitive,
                }
            }
            Fields::Named(_) => {
                quote! {
                    #enum_name::#variant_name{..} => #is_primitive,
                }
            }
        }
    });

    let expanded = quote! {
        impl #impl_generics #enum_name #ty_generics #where_clause {
            /// Returns true if this is a primitive type.
            pub const fn is_primitive(&self) -> bool {
                match self {
                    #(#is_primitive_items)*
                }
            }
        }
    };

    expanded.into()
}
