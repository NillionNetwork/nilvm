//! Test values used to generate test programs and run them.

use itertools::Itertools;

use operations::types::{DataType, Identifier, Literal};

#[derive(Debug, PartialEq)]
pub enum Error {
    ExhaustedAllValues,
}

struct Inputs {
    values: Vec<String>,
    index: usize,
}

impl Inputs {
    fn new(values: &[&str]) -> Self {
        // Note: if we decide to shuffle the values we could do it here.
        Self { values: values.iter().map(|value| value.to_string()).collect_vec(), index: 0 }
    }

    fn provide(&mut self) -> Result<String, Error> {
        let result = self.values.get(self.index).ok_or(Error::ExhaustedAllValues)?;

        self.index += 1;

        Ok(result.clone())
    }
}

pub struct InputProvider {
    signed_integer_values: Inputs,
    unsigned_integer_values: Inputs,
}

impl InputProvider {
    pub fn new() -> Self {
        Self {
            signed_integer_values: Inputs::new(&["-2", "-3", "1", "0", "4", "5"]),
            unsigned_integer_values: Inputs::new(&["0", "2", "3", "1", "4"]),
        }
    }

    pub fn provide(&mut self, data_type: DataType) -> Result<String, Error> {
        let permutations = match data_type {
            DataType::Literal(Literal::Integer)
            | DataType::Identifier(Identifier::Integer)
            | DataType::Identifier(Identifier::SecretInteger)
            | DataType::Identifier(Identifier::ShamirShareInteger) => &mut self.signed_integer_values,
            DataType::Literal(Literal::UnsignedInteger)
            | DataType::Identifier(Identifier::UnsignedInteger)
            | DataType::Identifier(Identifier::SecretUnsignedInteger)
            | DataType::Identifier(Identifier::ShamirShareUnsignedInteger) => &mut self.unsigned_integer_values,
            _ => panic!("input_provider: unsupported data type"),
        };

        permutations.provide()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_provider() {
        let mut provider = InputProvider::new();

        let mut values = vec![];

        for _ in 0..provider.unsigned_integer_values.values.len() {
            values.push(provider.provide(DataType::Literal(Literal::UnsignedInteger)).unwrap());
        }

        assert!(values.contains(&"0".to_string()));
        assert!(values.contains(&"2".to_string()));
        assert!(values.contains(&"3".to_string()));
        assert_eq!(provider.provide(DataType::Literal(Literal::UnsignedInteger)), Err(Error::ExhaustedAllValues));
    }
}
