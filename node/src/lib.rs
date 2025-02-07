//! The Nillion node.

#![deny(missing_docs)]
#![cfg_attr(not(test), forbid(unsafe_code))]
#![deny(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::iterator_step_by_zero,
    clippy::invalid_regex,
    clippy::string_slice,
    clippy::unimplemented,
    clippy::todo
)]
#![feature(assert_matches)]
#![feature(let_chains)]

pub mod builder;
pub(crate) mod channels;
pub mod controllers;
pub(crate) mod grpc;
pub mod observability;
pub mod services;
pub(crate) mod stateful;
pub mod storage;

pub use node_config as config;

use node_api::preprocessing::rust::PreprocessingElement;

pub(crate) trait PreprocessingConfigExt {
    /// Get the batch size for a preprocessing element.
    fn batch_size(&self, element: &PreprocessingElement) -> u64;

    /// Get the config for a prepreocessing element.
    fn element_config(&self, element: &PreprocessingElement) -> &config::PreprocessingProtocolConfig;
}

impl PreprocessingConfigExt for config::PreprocessingConfig {
    fn batch_size(&self, element: &PreprocessingElement) -> u64 {
        let config = self.element_config(element);
        config.batch_size
    }

    fn element_config(&self, element: &PreprocessingElement) -> &config::PreprocessingProtocolConfig {
        match element {
            PreprocessingElement::Compare => &self.compare,
            PreprocessingElement::DivisionSecretDivisor => &self.division_integer_secret,
            PreprocessingElement::Modulo => &self.modulo,
            PreprocessingElement::EqualityPublicOutput => &self.public_output_equality,
            PreprocessingElement::TruncPr => &self.truncpr,
            PreprocessingElement::Trunc => &self.trunc,
            PreprocessingElement::EqualitySecretOutput => &self.equals_integer_secret,
            PreprocessingElement::RandomInteger => &self.random_integer,
            PreprocessingElement::RandomBoolean => &self.random_boolean,
        }
    }
}
