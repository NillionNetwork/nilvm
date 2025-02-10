//! Nada value classification utilities.

use crate::{
    clear::Clear,
    encoders::blob_chunk_size,
    encrypted::{Encoded, Encrypted},
    NadaValue,
};
use math_lib::modular::SafePrime;
use nada_type::PrimitiveTypes;
use std::collections::HashMap;

/// A classification of nada values.
pub struct NadaValuesClassification {
    /// The number of shares
    pub shares: u64,

    /// The number of public values
    pub public: u64,

    /// The number of ecdsa key shares
    pub ecdsa_private_key_shares: u64,

    /// The number of ecdsa signatures shares
    pub ecdsa_signature_shares: u64,
}

impl NadaValuesClassification {
    /// Classify a set nada values in clear.
    pub fn new_from_clear<T: SafePrime>(values: &HashMap<String, NadaValue<Clear>>) -> Self {
        Self::new(values, |blob| blob.len().div_ceil(blob_chunk_size::<T>()) as u64)
    }

    /// Classify a set of nada values in encrypted form.
    pub fn new_from_encrypted(values: &HashMap<String, NadaValue<Encrypted<Encoded>>>) -> Self {
        Self::new(values, |blob| blob.value.len() as u64)
    }

    fn new<U>(values: &HashMap<String, NadaValue<U>>, process_blob: fn(&U::SecretBlob) -> u64) -> Self
    where
        U: PrimitiveTypes,
    {
        let mut shares = 0u64;
        let mut public = 0u64;
        let mut ecdsa_private_key_shares = 0u64;
        let mut ecdsa_signature_shares = 0u64;
        for value in values.values() {
            match value {
                NadaValue::SecretBlob(blob) => {
                    let shares_count = process_blob(blob);
                    shares = shares.saturating_add(shares_count);
                }
                _ => {
                    let count = value.to_type().elements_count();
                    if let Ok(count) = count {
                        shares = shares.saturating_add(count.share as u64);
                        public = public.saturating_add(count.public as u64);
                        ecdsa_private_key_shares =
                            ecdsa_private_key_shares.saturating_add(count.ecdsa_private_key_shares as u64);
                        ecdsa_signature_shares =
                            ecdsa_signature_shares.saturating_add(count.ecdsa_signature_shares as u64);
                    }
                }
            };
        }

        Self { shares, public, ecdsa_private_key_shares, ecdsa_signature_shares }
    }
}
