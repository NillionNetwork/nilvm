use heck::ToSnakeCase;
use proc_macro::TokenStream;
use proc_macro2::{Literal, Span};
use quote::quote;
use syn::{parse_macro_input, Data, DeriveInput, Fields, Ident};

use crate::helpers::{generate_tuple_field_name, get_variant_attribute};

/// Generates a new_* function for each enum variant.
/// Use the `skip_new_function` attribute to skip new function generation.
pub(crate) fn generate_enum_new_functions_impl(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let enum_name = input.ident;
    let Data::Enum(data_enum) = input.data else {
        panic!("{} is not an enum", enum_name);
    };
    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();

    let new_items = data_enum.variants.iter().map(|variant| {
        if get_variant_attribute(variant, "skip_new_function").is_some() {
            return quote! {};
        }

        let variant_name = &variant.ident;
        let function_name =
            Ident::new(&format!("new_{}", variant_name.to_string().to_snake_case()), variant_name.span());
        let comment = Literal::string(&format!("/// Returns a new {}.", variant_name));

        match &variant.fields {
            Fields::Unit => {
                quote! {
                    #[doc = #comment]
                    pub fn #function_name() -> Self {
                        #enum_name::#variant_name
                    }
                }
            }
            Fields::Unnamed(fields) => {
                let generics = fields.unnamed.iter().enumerate().map(|(i, field)| {
                    let ty = &field.ty;
                    let generic_name = Ident::new(&format!("U{i}"), Span::call_site());

                    quote! { #generic_name: Into<#ty> }
                });
                let parameters_and_types = fields.unnamed.iter().enumerate().map(|(i, _)| {
                    let param_name =
                        Ident::new(&generate_tuple_field_name(i, fields.unnamed.len()), variant_name.span());
                    let generic_name = Ident::new(&format!("U{i}"), Span::call_site());

                    quote! { #param_name: #generic_name }
                });
                let parameters = fields.unnamed.iter().enumerate().map(|(i, _)| {
                    let param_name =
                        Ident::new(&generate_tuple_field_name(i, fields.unnamed.len()), variant_name.span());

                    quote! { #param_name.into() }
                });
                quote! {
                    #[doc = #comment]
                    pub fn #function_name<#(#generics),*>(#(#parameters_and_types),*) -> Self {
                        #enum_name::#variant_name(#(#parameters),*)
                    }
                }
            }
            Fields::Named(fields) => {
                let parameters_and_types = fields.named.iter().map(|field| {
                    let ident = field.ident.as_ref().unwrap();
                    let ty = &field.ty;
                    quote! { #ident: #ty }
                });
                let parameters = fields.named.iter().map(|field| {
                    let ident = field.ident.as_ref().unwrap();
                    quote! { #ident }
                });
                quote! {
                    #[doc = #comment]
                    pub fn #function_name(#(#parameters_and_types),*) -> Self {
                        #enum_name::#variant_name { #(#parameters),* }
                    }
                }
            }
        }
    });

    let expanded = quote! {
        impl #impl_generics #enum_name #ty_generics #where_clause {
            #(#new_items)*
        }
    };

    expanded.into()
}
