//! The ECDSA auxiliary information fake generation.

use super::output::{EcdsaAuxInfo, EcdsaAuxInfoOutput};
use anyhow::Result;
use cggmp21::{
    key_share::{DirtyAuxInfo, PartyAux, Validate},
    paillier_zk::rug::Integer,
    security_level::SecurityLevel128,
};
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, marker::PhantomData};
use thiserror::Error;

const ECDSA_CURVE_KEY: &str = "curve=secp256k1,hd_wallet=false";

#[allow(dead_code)]
static CACHED_AUX_INFO: Lazy<FakeEcdsaAuxInfo> = Lazy::new(|| {
    serde_json::from_str(include_str!("./fake-data/fake_aux_info.json")).unwrap_or_else(|_| {
        eprintln!("Failed to deserialize precomputed_aux_info, using default value.");
        FakeEcdsaAuxInfo::default()
    })
});

/// A struct that holds fake threshold ecdsa auxiliary information
#[derive(Default, Deserialize)]
pub struct FakeEcdsaAuxInfo(HashMap<String, BaseDirtyAuxInfo>);

/// All fake dirty aux info will be based on this struct
#[derive(Clone, Serialize, Deserialize)]
#[serde(bound = "")]
struct BaseDirtyAuxInfo {
    /// Secret prime $p$
    p: Integer,
    /// Secret prime $q$
    q: Integer,
    /// Public auxiliary data of on party sharing the key
    party: PartyAux,
}

impl FakeEcdsaAuxInfo {
    /// Retrieves the auxiliary information for the given parameters.
    ///
    /// # Arguments
    /// * `n` - The number of parties.
    pub fn generate_ecdsa(n: u16) -> Result<EcdsaAuxInfoOutput<EcdsaAuxInfo>, FakeEcdsaAuxInfoError> {
        CACHED_AUX_INFO.ecdsa_aux_info_output(n)
    }

    /// Retrieves the auxiliary information for the given parameters.
    ///
    /// # Arguments
    /// * `n` - The number of parties.
    pub fn ecdsa_aux_info_output(&self, n: u16) -> Result<EcdsaAuxInfoOutput<EcdsaAuxInfo>, FakeEcdsaAuxInfoError> {
        let base_aux_info = self.0.get(ECDSA_CURVE_KEY).ok_or(FakeEcdsaAuxInfoError::KeyNotFound)?;
        let vec_party_aux = (0..n).map(|_| base_aux_info.party.clone()).collect();
        let dirty_aux_info = DirtyAuxInfo {
            p: base_aux_info.p.clone(),
            q: base_aux_info.q.clone(),
            parties: vec_party_aux,
            security_level: PhantomData::<SecurityLevel128>,
        };

        let aux_info = dirty_aux_info.validate().map_err(|e| FakeEcdsaAuxInfoError::ValidationError(e.to_string()))?;
        let aux_info_output = EcdsaAuxInfoOutput::Success { element: EcdsaAuxInfo { aux_info } };
        Ok(aux_info_output)
    }
}

/// Fake ecdsa aux info error
#[derive(Error, Debug)]
pub enum FakeEcdsaAuxInfoError {
    /// Error when the auxiliary information key is not found in the map.
    #[error("auxiliary information key not found")]
    KeyNotFound,

    /// General error for handling failures when processing auxiliary information.
    #[error("validation failed: {0}")]
    ValidationError(String),
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_get_aux_info_party() {
        let cached_aux_info = &CACHED_AUX_INFO;
        let n = 3;
        let result = cached_aux_info.ecdsa_aux_info_output(n);
        result.expect("invalid aux info");
    }
}
