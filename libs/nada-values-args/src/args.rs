//! Clap argument helpers.

use crate::{file::Inputs, parse::Parse};
use clap::Args;
use nada_value::{clear::Clear, NadaType, NadaValue};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, iter};

/// An argument wrapper that contains secrets.
#[derive(Args, Deserialize, Serialize, Debug, PartialEq)]
pub struct NadaValueArgs {
    /// An integer public variable.
    ///
    /// These must follow the pattern `<name>=<value>`.
    #[clap(long = "public-integer", short = 'i')]
    pub integers: Vec<String>,

    /// An unsigned integer public variable.
    ///
    /// These must follow the pattern `<name>=<value>`.
    #[clap(long = "public-unsigned-integer", visible_alias = "ui")]
    pub unsigned_integers: Vec<String>,

    /// An integer secret.
    ///
    /// These must follow the pattern `<name>=<value>`.
    #[clap(long = "secret-integer", visible_alias = "si")]
    pub secret_integers: Vec<String>,

    /// An unsigned integer secret.
    ///
    /// These must follow the pattern `<name>=<value>`.
    #[clap(long = "secret-unsigned-integer", visible_alias = "sui")]
    pub secret_unsigned_integers: Vec<String>,

    /// An array of integer public variables
    ///
    /// The expected pattern is `<name>=<comma-separated-value>`.
    ///
    /// Example: array1=1,2,3
    #[clap(long = "array-public-integer", visible_alias = "ai")]
    pub array_integers: Vec<String>,

    /// An array of unsigned integer public variables
    ///
    /// The expected pattern is `<name>=<comma-separated-value>`.
    ///
    /// Example: array1=1,2,3
    #[clap(long = "array-public-unsigned-integer", visible_alias = "aui")]
    pub array_unsigned_integers: Vec<String>,

    /// An array of integer secrets
    ///
    /// The expected pattern is `<name>=<comma-separated-value>`.
    ///
    /// Example: array1=1,2,3
    #[clap(long = "array-secret-integer", visible_alias = "asi")]
    pub array_secret_integers: Vec<String>,

    /// An array of unsigned integer secrets
    ///
    /// The expected pattern is `<name>=<comma-separated-value>`.
    ///
    /// Example: array1=1,2,3
    #[clap(long = "array-secret-unsigned-integer", visible_alias = "asui")]
    pub array_secret_unsigned_integers: Vec<String>,

    /// A blob secret.
    ///
    /// These must follow the pattern `<name>=<value>` and the value must be encoded in base64.
    #[clap(long = "secret-blob", visible_alias = "sb")]
    pub secret_blobs: Vec<String>,

    /// ECDSA message digests.
    ///
    /// These must follow the pattern `<name>=<value>` and the value must be encoded in base64.
    #[clap(long = "ecdsa-digest-message", visible_alias = "edm")]
    pub ecdsa_digest_messages: Vec<String>,

    /// A path to load secrets from.
    #[clap(long = "nada-values-path")]
    pub nada_values_path: Option<String>,
}

impl NadaValueArgs {
    /// Collect all secrets.
    pub fn parse(&self) -> anyhow::Result<HashMap<String, NadaValue<Clear>>> {
        let values = iter::empty()
            .chain(NadaType::Integer.parse_all(&self.integers)?)
            .chain(NadaType::UnsignedInteger.parse_all(&self.unsigned_integers)?)
            .chain(NadaType::SecretInteger.parse_all(&self.secret_integers)?)
            .chain(NadaType::SecretUnsignedInteger.parse_all(&self.secret_unsigned_integers)?)
            .chain(
                NadaType::Array { inner_type: Box::new(NadaType::Integer), size: 0 }.parse_all(&self.array_integers)?,
            )
            .chain(
                NadaType::Array { inner_type: Box::new(NadaType::UnsignedInteger), size: 0 }
                    .parse_all(&self.array_unsigned_integers)?,
            )
            .chain(
                NadaType::Array { inner_type: Box::new(NadaType::SecretInteger), size: 0 }
                    .parse_all(&self.array_secret_integers)?,
            )
            .chain(
                NadaType::Array { inner_type: Box::new(NadaType::SecretUnsignedInteger), size: 0 }
                    .parse_all(&self.array_secret_unsigned_integers)?,
            )
            .chain(NadaType::SecretBlob.parse_all(&self.secret_blobs)?)
            .chain(NadaType::EcdsaDigestMessage.parse_all(&self.ecdsa_digest_messages)?);

        let mut values: HashMap<String, NadaValue<Clear>> = values.map(|secret| (secret.name, secret.value)).collect();

        if let Some(path) = &self.nada_values_path {
            let parsed = Inputs::load(path)?;
            let file_inputs = parsed.parse_values()?.map(|input| (input.name, input.value));
            values.extend(file_inputs);
        }
        Ok(values)
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn parse_secrets() {
        let args = NadaValueArgs {
            integers: vec!["a=1".to_string(), "b=2".to_string()],
            unsigned_integers: vec!["a=1".to_string(), "b=2".to_string()],
            secret_integers: vec!["a=1".to_string(), "b=2".to_string()],
            secret_unsigned_integers: vec!["c=3".to_string(), "d=4".to_string()],

            array_integers: vec!["e=1,2,3".to_string(), "f=4,5,6".to_string()],
            array_unsigned_integers: vec!["g=7,8,9".to_string(), "h=10,11,12".to_string()],
            array_secret_integers: vec!["e=1,2,3".to_string(), "f=4,5,6".to_string()],
            array_secret_unsigned_integers: vec!["g=7,8,9".to_string(), "h=10,11,12".to_string()],

            secret_blobs: vec!["i=Zm9v".to_string(), "j=YmFy".to_string()],
            ecdsa_digest_messages: vec!["k=AAECAwQFBgcICQoLDA0ODxAREhMUFRYXGBkaGxwdHh8=".to_string()],
            nada_values_path: None,
        };
        let secrets = args.parse();
        assert!(!secrets.is_err());
    }
}
