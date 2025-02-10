use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, punctuated::Punctuated, token::Comma, Data, DeriveInput, Fields, Ident, Variant};

fn generate_to<'a>(
    variants: &'a Punctuated<Variant, Comma>,
    enum_name: &'a Ident,
) -> impl Iterator<Item = proc_macro2::TokenStream> + 'a {
    variants.iter().map(move |variant| {
        let variant_name = &variant.ident;

        match &variant.fields {
            Fields::Unit => {
                quote! {
                    #enum_name::#variant_name => NadaTypeKind::#variant_name,
                }
            }
            Fields::Unnamed(_) => {
                quote! {
                    #enum_name::#variant_name(..) => NadaTypeKind::#variant_name,
                }
            }
            Fields::Named(_) => {
                quote! {
                    #enum_name::#variant_name{..} => NadaTypeKind::#variant_name,
                }
            }
        }
    })
}

/// Generates `to_nada_type` and `into_nada_type` functions for an enum.
pub(crate) fn generate_to_nada_type_kind_impl(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let enum_name = input.ident;
    let Data::Enum(data_enum) = input.data else {
        panic!("{} is not an enum", enum_name);
    };
    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();

    let to_type_items = generate_to(&data_enum.variants, &enum_name);
    let into_type_items = generate_to(&data_enum.variants, &enum_name);

    let expanded = quote! {
        impl #impl_generics #enum_name #ty_generics #where_clause {
            /// Returns this variant's type kind.
            pub fn to_type_kind(&self) -> NadaTypeKind {
                match self {
                    #(#to_type_items)*
                }
            }

            /// Returns this variant's type kind.
            pub fn into_type_kind(self) -> NadaTypeKind {
                match self {
                    #(#into_type_items)*
                }
            }
        }
    };

    expanded.into()
}
