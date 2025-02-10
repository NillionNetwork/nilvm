//! State machine derivation macros.

#![deny(missing_docs)]
#![forbid(unsafe_code)]
#![deny(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::iterator_step_by_zero,
    clippy::invalid_regex,
    clippy::string_slice,
    clippy::unimplemented,
    clippy::todo
)]

use heck::ToSnakeCase;
use proc_macro2::{Ident, Span, TokenStream};
use quote::quote;
use syn::{
    parse_macro_input, spanned::Spanned, DataEnum, DeriveInput, Error, Field, ImplGenerics, Lit, TypeGenerics, Variant,
    WhereClause,
};

enum AccessType {
    Ref,
    RefMut,
}

struct AccessorMetadata {
    name: Ident,
    docstring: String,
    variant_name: Ident,
    access: AccessType,
}

impl AccessorMetadata {
    fn new(variant_name: &syn::Ident, access: AccessType) -> Self {
        let (raw_name, docstring) = match access {
            AccessType::Ref => (
                format!("{variant_name}_state"),
                format!("Get an immutable reference to the internal state if this is a {variant_name} state"),
            ),
            AccessType::RefMut => (
                format!("{variant_name}_state_mut"),
                format!("Get a mutable reference to the internal state if this is a {variant_name} state"),
            ),
        };
        let name = Ident::new(&raw_name.to_snake_case(), Span::call_site());
        let variant_name = variant_name.clone();
        AccessorMetadata { name, docstring, variant_name, access }
    }

    fn new_ref(variant_name: &syn::Ident) -> Self {
        Self::new(variant_name, AccessType::Ref)
    }

    fn new_ref_mut(variant_name: &syn::Ident) -> Self {
        Self::new(variant_name, AccessType::RefMut)
    }

    fn make_accessor(&self, field: &syn::Field) -> TokenStream {
        let raw_return_type = &field.ty;
        let variant_name = &self.variant_name;
        let inner = quote!(inner);
        let error = quote!(state_machine::errors::InvalidStateError);

        let (self_type, return_type) = match self.access {
            AccessType::Ref => (quote!(&self), quote!(&#raw_return_type)),
            AccessType::RefMut => (quote!(&mut self), quote!(&mut #raw_return_type)),
        };
        let name = &self.name;
        let docstring = &self.docstring;

        quote!(
            #[doc = #docstring]
            #[inline]
            pub fn #name(#self_type) -> Result<#return_type, #error> {
                match self {
                    Self::#variant_name(#inner) => {
                        Ok(#inner)
                    }
                    _ => Err(#error)
                }
            }
        )
    }
}

struct UnitAccessorMetadata {
    name: Ident,
    docstring: String,
    variant_name: Ident,
}

impl UnitAccessorMetadata {
    fn new(variant_name: &syn::Ident) -> Self {
        let name = Ident::new(&format!("{variant_name}_state").to_snake_case(), Span::call_site());
        let docstring = format!("Check if this is a {variant_name} state");
        let variant_name = variant_name.clone();
        UnitAccessorMetadata { name, docstring, variant_name }
    }

    fn make_accessor(&self) -> TokenStream {
        let variant_name = &self.variant_name;
        let error = quote!(state_machine::errors::InvalidStateError);

        let name = &self.name;
        let docstring = &self.docstring;

        quote!(
        #[doc = #docstring]
        #[inline]
        pub fn #name(&self) -> Result<(), #error> {
            match self {
                Self::#variant_name =>  Ok(()),
                _ => Err(#error)
            }
        })
    }
}

// Represents our attributes for a specific enum variant.
struct StateAttributes<'a> {
    completed_expr: Option<TokenStream>,
    transition_fn: Option<TokenStream>,
    submachine: Option<TokenStream>,
    contents: Option<&'a Field>,
    name: &'a Ident,
    immutable_access_branch_match: TokenStream,
    mutable_access_branch_match: TokenStream,
}

impl<'a> StateAttributes<'a> {
    /// Parse attributes for a specific variant in the enum.
    fn parse(enum_name: &syn::Ident, variant: &'a syn::Variant) -> syn::Result<StateAttributes<'a>> {
        let mut completed_expr = None;
        let mut transition_fn = None;
        let mut submachine = None;
        let contents = Self::get_contents(variant)?;
        let name = &variant.ident;
        for attribute in &variant.attrs {
            if !attribute.path().is_ident("state_machine") {
                continue;
            }
            attribute.parse_nested_meta(|meta| {
                if meta.path.is_ident("completed") {
                    let value: Lit = meta.value()?.parse()?;
                    completed_expr = Some(parse_lit(meta.path.span(), &value)?);
                    Ok(())
                } else if meta.path.is_ident("completed_fn") {
                    let value: Lit = meta.value()?.parse()?;
                    let lit = parse_lit(meta.path.span(), &value)?;
                    completed_expr = Some(quote!(#lit(state)));
                    Ok(())
                } else if meta.path.is_ident("transition_fn") {
                    let value: Lit = meta.value()?.parse()?;
                    transition_fn = Some(parse_lit(meta.path.span(), &value)?);
                    Ok(())
                } else if meta.path.is_ident("submachine") {
                    let value: Lit = meta.value()?.parse()?;
                    submachine = Some(parse_lit(meta.path.span(), &value)?);
                    Ok(())
                } else {
                    Err(Error::new(meta.path.span(), "unexpected attribute"))
                }
            })?;
        }

        // Construct different types of branches to match/ignore each branch depending on whether it's a variant
        // that contains data or it's a unit one.
        let (immutable_access_branch_match, mutable_access_branch_match) = match contents {
            Some(_) => (quote!(#enum_name::#name(state)), quote!(#enum_name::#name(mut state))),
            None => {
                let matcher = quote!(#enum_name::#name);
                (matcher.clone(), matcher)
            }
        };
        Ok(StateAttributes {
            completed_expr,
            transition_fn,
            submachine,
            contents,
            name,
            immutable_access_branch_match,
            mutable_access_branch_match,
        })
    }

    // Creates the branch for this enum variant in `StateMachineState::is_complete`.
    fn make_completed_branch(&self, variant: &syn::Variant) -> syn::Result<TokenStream> {
        match &self.completed_expr {
            Some(expr) => {
                let matcher = &self.immutable_access_branch_match;
                Ok(quote!(#matcher => #expr,))
            }
            None => match &self.submachine {
                Some(expr) => {
                    let matcher = &self.immutable_access_branch_match;
                    Ok(quote!(#matcher => #expr.is_finished(),))
                }
                None => Err(Error::new(variant.span(), "completion condition or submachine is missing")),
            },
        }
    }

    // Creates the branch for this enum variant in `StateMachineState::try_next`.
    fn make_transition_fn_branch(&self, variant: &syn::Variant) -> syn::Result<TokenStream> {
        match &self.transition_fn {
            Some(expr) => {
                let matcher = &self.mutable_access_branch_match;
                Ok(quote!(#matcher => Ok((#expr)(state)?),))
            }
            None => Err(Error::new(variant.span(), "completion condition is missing")),
        }
    }

    // Creates all accessors for this field.
    fn make_accessors(&self) -> syn::Result<TokenStream> {
        let mut tokens = TokenStream::new();
        let variant_name = &self.name;
        match self.contents {
            Some(inner) => {
                tokens.extend(AccessorMetadata::new_ref(variant_name).make_accessor(inner));
                tokens.extend(AccessorMetadata::new_ref_mut(variant_name).make_accessor(inner));
            }
            None => tokens.extend(UnitAccessorMetadata::new(variant_name).make_accessor()),
        }

        Ok(tokens)
    }

    // Safely unwraps the contents of this enum variant.
    #[allow(clippy::indexing_slicing)]
    fn get_contents(variant: &Variant) -> syn::Result<Option<&Field>> {
        match &variant.fields {
            syn::Fields::Unnamed(inner) if inner.unnamed.len() == 1 => Ok(Some(&inner.unnamed[0])),
            syn::Fields::Unnamed(_) => Err(Error::new(variant.span(), "only one inner state supported")),
            syn::Fields::Unit => Ok(None),
            syn::Fields::Named(_) => Err(Error::new(variant.span(), "named variants not supported")),
        }
    }
}

// Parses the contents of a string literal.
fn parse_lit(span: Span, lit: &Lit) -> syn::Result<TokenStream> {
    match lit {
        Lit::Str(lit) => lit.parse(),
        _ => Err(Error::new(span, "expected literal string")),
    }
}

// The properties of the enum itself.
struct EnumProperties {
    recipient_id: TokenStream,
    input_message: TokenStream,
    output_message: TokenStream,
    final_result: TokenStream,
    handle_message_fn: TokenStream,
}

impl EnumProperties {
    fn split_for_impl(self) -> (TokenStream, TokenStream, TokenStream, TokenStream, TokenStream) {
        (self.recipient_id, self.input_message, self.output_message, self.final_result, self.handle_message_fn)
    }
}

fn parse_enum_properties(input: &DeriveInput) -> syn::Result<EnumProperties> {
    let mut recipient_id = None;
    let mut input_message = None;
    let mut output_message = None;
    let mut final_result = None;
    let mut handle_message_fn = None;
    for attribute in &input.attrs {
        if !attribute.path().is_ident("state_machine") {
            continue;
        }
        attribute.parse_nested_meta(|meta| {
            if meta.path.is_ident("recipient_id") {
                let value: Lit = meta.value()?.parse()?;
                recipient_id = Some(parse_lit(meta.path.span(), &value)?);
                Ok(())
            } else if meta.path.is_ident("message") {
                let value: Lit = meta.value()?.parse()?;
                input_message.clone_from(&Some(parse_lit(meta.path.span(), &value)?));
                output_message = Some(parse_lit(meta.path.span(), &value)?);
                Ok(())
            } else if meta.path.is_ident("input_message") {
                let value: Lit = meta.value()?.parse()?;
                input_message.clone_from(&Some(parse_lit(meta.path.span(), &value)?));
                Ok(())
            } else if meta.path.is_ident("output_message") {
                let value: Lit = meta.value()?.parse()?;
                output_message = Some(parse_lit(meta.path.span(), &value)?);
                Ok(())
            } else if meta.path.is_ident("final_result") {
                let value: Lit = meta.value()?.parse()?;
                final_result = Some(parse_lit(meta.path.span(), &value)?);
                Ok(())
            } else if meta.path.is_ident("handle_message_fn") {
                let value: Lit = meta.value()?.parse()?;
                handle_message_fn = Some(parse_lit(meta.path.span(), &value)?);
                Ok(())
            } else {
                Err(Error::new(meta.path.span(), "unexpected attribute"))
            }
        })?;
    }
    // TODO: eventually enforce these are set.
    let properties = EnumProperties {
        recipient_id: recipient_id.unwrap_or_else(|| quote!(())),
        input_message: input_message.unwrap_or_else(|| quote!(())),
        output_message: output_message.unwrap_or_else(|| quote!(())),
        final_result: final_result.unwrap_or_else(|| quote!(())),
        handle_message_fn: handle_message_fn
            .unwrap_or_else(|| quote!(|state, _| Ok(state_machine::state::StateMachineStateOutput::Empty(state)))),
    };
    Ok(properties)
}

fn process_input(input: &DeriveInput) -> syn::Result<TokenStream> {
    let enum_name = &input.ident;
    let generics = &input.generics;

    let enum_data = match &input.data {
        syn::Data::Enum(data) => Ok(data),
        _ => Err(Error::new(input.span(), "macro only works on enums")),
    }?;

    if enum_data.variants.is_empty() {
        return Err(Error::new(input.span(), "enum has no variants"));
    }

    let properties = parse_enum_properties(input)?;
    let mut accessors = TokenStream::new();
    let mut completed_branches = TokenStream::new();
    let mut transition_fn_branches = TokenStream::new();
    for variant_data in &enum_data.variants {
        let attributes = StateAttributes::parse(enum_name, variant_data)?;

        // Add all accessors for this variant
        accessors.extend(attributes.make_accessors()?);

        // Build all branches
        completed_branches.extend(attributes.make_completed_branch(variant_data)?);
        transition_fn_branches.extend(attributes.make_transition_fn_branch(variant_data)?);
    }

    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();
    let (recipient_id, input_message, output_message, final_result, handle_message_fn) = properties.split_for_impl();

    let impl_display = display_impl(enum_name, &impl_generics, &ty_generics, &where_clause, enum_data)?;

    Ok(quote!(
        #[automatically_derived]
        impl #impl_generics #enum_name #ty_generics #where_clause {
            #accessors
        }

        #[automatically_derived]
        impl #impl_generics state_machine::StateMachineState for #enum_name #ty_generics #where_clause {
                type RecipientId = #recipient_id;
                type InputMessage = #input_message;
                type OutputMessage = #output_message;
                type FinalResult = #final_result;

                fn is_completed(&self) -> bool {
                    match self {
                        #completed_branches
                    }
                }

                fn try_next(mut self) -> state_machine::state::StateMachineStateResult<Self> {
                    match self {
                        #transition_fn_branches
                    }
                }

                fn handle_message(
                    self,
                    message: Self::InputMessage,
                ) -> state_machine::state::StateMachineStateResult<Self> {
                    (#handle_message_fn)(self, message)
                }
        }

        #impl_display

    ))
}

fn display_impl(
    enum_name: &Ident,
    impl_generics: &ImplGenerics,
    ty_generics: &TypeGenerics,
    where_clause: &Option<&WhereClause>,
    enum_data: &DataEnum,
) -> Result<TokenStream, syn::Error> {
    let enum_name_str = enum_name.to_string();
    let variants = enum_data
        .variants
        .iter()
        .map(|variant| {
            let ident = variant.ident.clone();
            let ident_str = ident.to_string();
            let attributes = StateAttributes::parse(enum_name, variant)?;

            match &variant.fields {
                syn::Fields::Unnamed(inner) if inner.unnamed.len() == 1 => {
                    if let Some(submachine) = attributes.submachine {
                        Ok(quote! {
                            #ident(state) => format!("{}::{}[{}]", #enum_name_str, #ident_str, #submachine),
                        })
                    } else {
                        Ok(quote! {
                        #ident(..) => format!("{}::{}", #enum_name_str, #ident_str),
                        })
                    }
                }
                syn::Fields::Unnamed(_) => Err(Error::new(variant.span(), "only one inner state supported")),
                syn::Fields::Unit => Ok(quote! {
                        #ident(..) => format!("{}::{}", #enum_name_str, #ident_str),
                }),
                syn::Fields::Named(_) => Err(Error::new(variant.span(), "named variants not supported")),
            }
        })
        .collect::<Result<Vec<_>, syn::Error>>()?;

    let output = quote! {
        #[automatically_derived]
        #[allow(unused_qualifications)]
        impl #impl_generics ::core::fmt::Display for #enum_name #ty_generics #where_clause {
            fn fmt(&self, f: &mut ::core::fmt::Formatter) -> ::core::fmt::Result {
                ::core::fmt::Formatter::write_str(
                    f,
                    &match self {
                        #(#enum_name::#variants)*
                    },
                )
            }
        }
    };

    Ok(output)
}

/// Entry point for the state machine state derivation.
///
/// This will create all accessors for a state machine state. That is, for a variant xyz in the enum that this
/// derive macro is being applied to, this will create:
///
/// * An `xyz_state` accessor that returns a `Result<&xyz, InvalidStateError>`.
/// * An `xyz_state_mut` accessor that returns a `Result<&mut xyz, InvalidStateError>`.
///
/// Besides that, this macro auto implements the `StateMachineState` trait for this type. See the examples below
/// for more information on usage:
///
/// ```
/// use state_machine::{
///     StateMachineState,
///     StateMachine,
///     StateMachineStateOutput,
///     StateMachineStateResult,
///     errors::StateMachineError,
/// };
///
/// struct FirstOneSubMachine;
/// struct SecondOneSubMachine;
///
/// #[derive(state_machine_derive::StateMachineState)]
/// #[state_machine(final_result = "String")]
/// enum SubMachine {
///     #[state_machine(completed = "true", transition_fn = "transition_first_one_submachine")]
///     FirstOne(FirstOneSubMachine),
///
///     #[state_machine(completed = "true", transition_fn = "transition_second_one_submachine")]
///     SecondOne(SecondOneSubMachine),
/// }
///
/// fn transition_first_one_submachine(_state: FirstOneSubMachine) -> StateMachineStateResult<SubMachine> {
///     Ok(StateMachineStateOutput::Empty(SubMachine::SecondOne(SecondOneSubMachine)))
/// }
///
/// fn transition_second_one_submachine(_state: SecondOneSubMachine) -> StateMachineStateResult<SubMachine> {
///     Ok(StateMachineStateOutput::Final("hello from submachine".to_string()))
/// }
///
/// struct FirstOne;
/// struct SecondOne;
/// struct ThirdOne { my_submachine: StateMachine<SubMachine> };
///
/// #[derive(state_machine_derive::StateMachineState)]
/// #[state_machine(final_result = "String")]
/// enum MyState {
///     #[state_machine(completed = "true", transition_fn = "transition_first_one")]
///     FirstOne(FirstOne),
///
///     #[state_machine(completed_fn = "returns_true", transition_fn = "transition_second_one")]
///     SecondOne(SecondOne),
///
///     #[state_machine(submachine = "state.my_submachine", transition_fn = "transition_third_one")]
///     ThirdOne(ThirdOne),
/// }
///
/// fn returns_true(_: &SecondOne) -> bool {
///     true
/// }
///
/// fn transition_first_one(_state: FirstOne) -> StateMachineStateResult<MyState> {
///     Ok(MyState::SecondOne(SecondOne).into())
/// }
///
/// fn transition_second_one(_state: SecondOne) -> StateMachineStateResult<MyState> {
///     Ok(MyState::ThirdOne(ThirdOne { my_submachine: StateMachine::new(SubMachine::FirstOne(FirstOneSubMachine))}).into())
/// }
///
/// fn transition_third_one(_state: ThirdOne) -> StateMachineStateResult<MyState> {
///     Ok(StateMachineStateOutput::Final("hello from main machine".to_string()))
/// }
///
/// let mut state = MyState::FirstOne(FirstOne);
/// assert!(state.first_one_state().is_ok());
/// assert!(state.first_one_state_mut().is_ok());
/// assert!(state.is_completed());
///
/// assert!(state.second_one_state().is_err());
/// ```
#[proc_macro_derive(StateMachineState, attributes(state_machine))]
pub fn state_machine_state_derive(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let expanded = process_input(&input).unwrap_or_else(syn::Error::into_compile_error);
    proc_macro::TokenStream::from(expanded)
}
