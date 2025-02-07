use proc_macro::TokenStream;
use quote::quote;
use syn::{
    parse_macro_input, punctuated::Punctuated, token::Comma, Data, DeriveInput, Expr, Fields, Ident, Meta, Token,
    Variant,
};

use crate::helpers::{generate_tuple_field_name, get_variant_attribute};

fn generate_to<'a>(
    variants: &'a Punctuated<Variant, Comma>,
    enum_name: &'a Ident,
    func_name: &'a str,
) -> impl Iterator<Item = proc_macro2::TokenStream> + 'a {
    variants.iter().map(move |variant| {
        let variant_name = &variant.ident;

        let mut to_function_name = None;

        if let Some(attr) = get_variant_attribute(variant, "to_type_functions") {
            let args = attr.parse_args_with(Punctuated::<Meta, Token![,]>::parse_terminated).unwrap();

            for meta in args {
                if let Meta::NameValue(name_value) = &meta {
                    if name_value.path.is_ident(func_name) {
                        if let Expr::Path(path) = &name_value.value {
                            if let Some(ident) = path.path.get_ident() {
                                to_function_name = Some(ident.to_string());
                            }
                        }
                    }
                }
            }
        }

        if let Some(to_function_name) = to_function_name {
            let function_name = Ident::new(&to_function_name, variant_name.span());

            let parameters_items = {
                let variant_name = &variant.ident;

                match &variant.fields {
                    Fields::Unit => {
                        quote! {}
                    }
                    Fields::Unnamed(fields) => {
                        let parameters = fields.unnamed.iter().enumerate().map(|(i, _)| {
                            let param_name =
                                Ident::new(&generate_tuple_field_name(i, fields.unnamed.len()), variant_name.span());
                            quote! { #param_name }
                        });
                        quote! {
                            #(#parameters),*
                        }
                    }
                    Fields::Named(fields) => {
                        let parameters = fields.named.iter().map(|field| {
                            let ident = field.ident.as_ref().unwrap();
                            quote! { #ident }
                        });
                        quote! {
                            #(#parameters),*
                        }
                    }
                }
            };

            match &variant.fields {
                Fields::Unit => {
                    quote! {
                        #enum_name::#variant_name => #function_name(),
                    }
                }
                Fields::Unnamed(_) => {
                    quote! {
                        #enum_name::#variant_name(#parameters_items) => #function_name(#parameters_items),
                    }
                }
                Fields::Named(_) => {
                    quote! {
                        #enum_name::#variant_name{#parameters_items} => #function_name(#parameters_items),
                    }
                }
            }
        } else {
            match &variant.fields {
                Fields::Unit => {
                    quote! {
                        #enum_name::#variant_name => NadaType::#variant_name,
                    }
                }
                Fields::Unnamed(_) => {
                    quote! {
                        #enum_name::#variant_name(..) => NadaType::#variant_name,
                    }
                }
                Fields::Named(_) => {
                    quote! {
                        #enum_name::#variant_name{..} => NadaType::#variant_name,
                    }
                }
            }
        }
    })
}

/// Generates `to_type` and `into_type` functions for an enum.
/// Use `to_type_functions(to_type = my_variant_to_type, into_type = my_variant_into_type)` to specify a function that
/// should be called instead of relying on the automatically generated one.
pub(crate) fn generate_to_nada_type_impl(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let enum_name = input.ident;
    let Data::Enum(data_enum) = input.data else {
        panic!("{} is not an enum", enum_name);
    };
    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();

    let to_type_items = generate_to(&data_enum.variants, &enum_name, "to_type");
    let into_type_items = generate_to(&data_enum.variants, &enum_name, "into_type");

    let expanded = quote! {
        impl #impl_generics #enum_name #ty_generics #where_clause {
            /// Returns this variant's type.
            pub fn to_type(&self) -> NadaType {
                match self {
                    #(#to_type_items)*
                }
            }

            /// Returns this variant's type.
            pub fn into_type(self) -> NadaType {
                match self {
                    #(#into_type_items)*
                }
            }
        }
    };

    expanded.into()
}
