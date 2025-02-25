use std::{collections::HashSet, str::FromStr, sync::LazyLock};

static LABEL_ALPHABET: LazyLock<HashSet<char>> =
    LazyLock::new(|| ('a'..='z').chain('A'..='Z').chain('0'..='9').chain(['_', '-']).collect());

/// A selector that can be applied to a NUC.
#[derive(Clone, Debug, PartialEq)]
pub struct Selector(Vec<String>);

impl Selector {
    pub fn apply<'a>(&self, mut value: &'a serde_json::Value) -> &'a serde_json::Value {
        for label in &self.0 {
            match value.get(label) {
                Some(inner) => value = inner,
                None => return &serde_json::Value::Null,
            };
        }
        value
    }
}

impl FromStr for Selector {
    type Err = SelectorParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if !s.starts_with(".") {
            return Err(SelectorParseError::LeadingDot)?;
        }
        let s = &s[1..];
        if s.is_empty() {
            return Ok(Self(vec![]));
        }

        let mut labels = Vec::new();
        for label in s.split('.') {
            if !label.chars().all(|c| LABEL_ALPHABET.contains(&c)) {
                return Err(SelectorParseError::Alphabet);
            } else if label.is_empty() {
                return Err(SelectorParseError::Empty);
            }
            labels.push(label.to_string());
        }
        Ok(Self(labels))
    }
}

/// An error encountered when parsing a selector.
#[derive(Debug, thiserror::Error)]
pub enum SelectorParseError {
    #[error("invalid attribute character")]
    Alphabet,

    #[error("empty attribute")]
    Empty,

    #[error("selector must start with '.'")]
    LeadingDot,
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;
    use serde_json::{json, Value};

    #[rstest]
    #[case::identity(".", &[])]
    #[case::single(".foo", &["foo"])]
    #[case::multi(".foo.bar", &["foo", "bar"])]
    #[case::entire_alphabet(".abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789-_", &["abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789-_"])]
    fn parse_valid_selectors(#[case] input: &str, #[case] path: &[&str]) {
        let parsed: Selector = input.parse().expect("parse failed");
        assert_eq!(parsed.0, path);
    }

    #[rstest]
    #[case::empty("")]
    #[case::no_leading_dot("A")]
    #[case::invalid_field_name1(".#")]
    #[case::invalid_field_name2(".🚀")]
    #[case::trailing_dot(".A.")]
    #[case::empty_label(".A..B")]
    fn parse_invalid_selectors(#[case] input: &str) {
        input.parse::<Selector>().expect_err("parse succeeded");
    }

    #[rstest]
    #[case::identity(".", json!({"foo": 42}), json!({"foo": 42}))]
    #[case::field(".foo", json!({"foo": 42}), json!(42))]
    #[case::nested(".foo.bar", json!({"foo": {"bar": 42}}), json!(42))]
    #[case::non_existent(".foo", json!({"bar": 42}), Value::Null)]
    fn lookup(#[case] expr: &str, #[case] input: Value, #[case] expected: Value) {
        let expr: Selector = expr.parse().expect("invalid expression");
        let output = expr.apply(&input);
        assert_eq!(output, &expected);
    }
}
