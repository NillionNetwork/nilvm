use proc_macro2::Span;
use syn::{Attribute, Ident, Variant};

/// Generates tuple field names depending on the number of fields and their index.
pub fn generate_tuple_field_name(index: usize, size: usize) -> String {
    match size {
        // If there is only one field, call it `value`.
        1 => "value".to_string(),
        // If there are two fields, call them `left` and `right`.
        2 => match index {
            0 => "left".to_string(),
            1 => "right".to_string(),
            _ => panic!("index has to be < size"),
        },
        // Otherwise just use valueX where X is the field index.
        _ => format!("value{}", index),
    }
}

/// Returns an Option with an attribute, if it is present.
pub fn get_variant_attribute<'a>(variant: &'a Variant, name: &str) -> Option<&'a Attribute> {
    let expected_ident = Ident::new(name, Span::call_site());

    variant.attrs.iter().find(|attr| attr.path().is_ident(&expected_ident))
}
