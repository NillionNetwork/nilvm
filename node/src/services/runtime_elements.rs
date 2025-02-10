//! Runtime elements service.

use super::{auxiliary_material::EcdaAuxInfoMaterialService, preprocessing::PreprocessingBlobService};
use crate::services::preprocessing::{
    PrepCompareSharesService, PrepDivisionIntegerSecretSharesService, PrepEqualsIntegerSecretSharesService,
    PrepModuloSharesService, PrepPublicOutputEqualitySharesService, PrepRandomBooleanSharesService,
    PrepRandomIntegerSharesService, PrepTruncPrSharesService, PrepTruncSharesService,
};
use async_trait::async_trait;
use core::fmt;
use metrics::prelude::*;
use mpc_vm::vm::plan::MPCRuntimePreprocessingElements;
use node_api::{
    payments::rust::SelectedAuxiliaryMaterial,
    preprocessing::rust::{AuxiliaryMaterial, PreprocessingElement},
};
use once_cell::sync::Lazy;
use std::{collections::HashMap, ops::Range, time::Duration};
use tracing::{error, info};

static METRICS: Lazy<Metrics> = Lazy::new(Metrics::default);

/// The runtime preprocessing elements plan.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct PreprocessingElementsPlan(pub(crate) HashMap<PreprocessingElement, PreprocessingElementOffsets>);

/// The offsets being used for preprocessing elements.
#[derive(Clone, Default, PartialEq)]
pub(crate) struct PreprocessingElementOffsets {
    /// The first batch id to be used.
    pub(crate) first_batch_id: u32,

    /// The last batch id to be used.
    pub(crate) last_batch_id: u32,

    /// The start offset for the first batch.
    pub(crate) start_offset: u32,

    /// The total number of elements.
    pub(crate) total: u32,
}

impl fmt::Debug for PreprocessingElementOffsets {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let Self { first_batch_id, last_batch_id, start_offset, total } = self;
        write!(f, "Offsets(total={total}, batches={first_batch_id}..={last_batch_id}, start_offset={start_offset})")
    }
}

impl PreprocessingElementOffsets {
    /// Constructs offsets from a range and batch size.
    #[allow(clippy::arithmetic_side_effects)]
    pub fn from_range(offset_range: Range<u64>, batch_size: u64) -> Self {
        let first_batch_id = offset_range.start.wrapping_div(batch_size);
        let last_batch_id = offset_range.end.saturating_sub(1).wrapping_div(batch_size);
        let start_offset = offset_range.start.saturating_sub(first_batch_id.wrapping_mul(batch_size));
        Self {
            first_batch_id: first_batch_id as u32,
            last_batch_id: last_batch_id as u32,
            start_offset: start_offset as u32,
            total: offset_range.count() as u32,
        }
    }
}

#[cfg_attr(test, mockall::automock)]
#[async_trait]
pub(crate) trait RuntimeElementsService: Send + Sync + 'static {
    async fn request_elements(
        &self,
        preprocessing_plan: PreprocessingElementsPlan,
        auxiliary_materials: &[SelectedAuxiliaryMaterial],
    ) -> Result<MPCRuntimePreprocessingElements, RequestError>;
}

/// The runtime elements service.
///
/// This service can be used to fetch the elements needed for online protocols like COMPARE, MODULO and others.
pub(crate) struct DefaultRuntimeElementsService {
    compare_service: PrepCompareSharesService,
    division_integer_secret_service: PrepDivisionIntegerSecretSharesService,
    modulo_service: PrepModuloSharesService,
    public_output_equality_service: PrepPublicOutputEqualitySharesService,
    equals_integer_secret_service: PrepEqualsIntegerSecretSharesService,
    truncpr_service: PrepTruncPrSharesService,
    trunc_service: PrepTruncSharesService,
    random_integer_service: PrepRandomIntegerSharesService,
    random_boolean_service: PrepRandomBooleanSharesService,
    ecdsa_aux_info_service: EcdaAuxInfoMaterialService,
}

impl DefaultRuntimeElementsService {
    /// Create a new runtime elements service.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        compare_service: PrepCompareSharesService,
        division_integer_secret_service: PrepDivisionIntegerSecretSharesService,
        modulo_service: PrepModuloSharesService,
        public_output_equality_service: PrepPublicOutputEqualitySharesService,
        equals_integer_secret_service: PrepEqualsIntegerSecretSharesService,
        truncpr_service: PrepTruncPrSharesService,
        trunc_service: PrepTruncSharesService,
        random_integer_service: PrepRandomIntegerSharesService,
        random_boolean_service: PrepRandomBooleanSharesService,
        ecdsa_aux_info_service: EcdaAuxInfoMaterialService,
    ) -> Self {
        Self {
            compare_service,
            division_integer_secret_service,
            modulo_service,
            public_output_equality_service,
            equals_integer_secret_service,
            truncpr_service,
            trunc_service,
            random_integer_service,
            random_boolean_service,
            ecdsa_aux_info_service,
        }
    }

    async fn find_elements<T>(
        element: PreprocessingElement,
        offsets: PreprocessingElementOffsets,
        service: &dyn PreprocessingBlobService<T>,
    ) -> Result<Vec<T>, RequestError>
    where
        T: Send + Sync + 'static,
    {
        match service.find_by_offsets(&offsets).await {
            Ok(shares) => Ok(shares),
            Err(e) => {
                error!("Failed to lookup {element} shares: {e}");
                Err(RequestError::Lookup)
            }
        }
    }
}

#[async_trait]
impl RuntimeElementsService for DefaultRuntimeElementsService {
    /// Request runtime elements.
    async fn request_elements(
        &self,
        preprocessing_plan: PreprocessingElementsPlan,
        auxiliary_materials: &[SelectedAuxiliaryMaterial],
    ) -> Result<MPCRuntimePreprocessingElements, RequestError> {
        let _timer = METRICS.request_elements_timer();

        info!("Requesting elements using plan: {preprocessing_plan:?}");
        let mut elements = MPCRuntimePreprocessingElements::default();
        for (element, offsets) in preprocessing_plan.0 {
            match element {
                PreprocessingElement::Compare => {
                    elements.compare = Self::find_elements(element, offsets, self.compare_service.as_ref()).await?
                }
                PreprocessingElement::DivisionSecretDivisor => {
                    elements.division_integer_secret =
                        Self::find_elements(element, offsets, self.division_integer_secret_service.as_ref()).await?
                }
                PreprocessingElement::EqualitySecretOutput => {
                    elements.equals_integer_secret =
                        Self::find_elements(element, offsets, self.equals_integer_secret_service.as_ref()).await?
                }
                PreprocessingElement::EqualityPublicOutput => {
                    elements.public_output_equality =
                        Self::find_elements(element, offsets, self.public_output_equality_service.as_ref()).await?
                }
                PreprocessingElement::Modulo => {
                    elements.modulo = Self::find_elements(element, offsets, self.modulo_service.as_ref()).await?
                }
                PreprocessingElement::Trunc => {
                    elements.trunc = Self::find_elements(element, offsets, self.trunc_service.as_ref()).await?
                }
                PreprocessingElement::TruncPr => {
                    elements.truncpr = Self::find_elements(element, offsets, self.truncpr_service.as_ref()).await?
                }
                PreprocessingElement::RandomInteger => {
                    elements.random_integer =
                        Self::find_elements(element, offsets, self.random_integer_service.as_ref()).await?
                }
                PreprocessingElement::RandomBoolean => {
                    elements.random_boolean =
                        Self::find_elements(element, offsets, self.random_boolean_service.as_ref()).await?
                }
            };
        }
        for selected_material in auxiliary_materials {
            let SelectedAuxiliaryMaterial { material, version } = selected_material;
            match material {
                AuxiliaryMaterial::Cggmp21AuxiliaryInfo => {
                    let material = self.ecdsa_aux_info_service.lookup(*version).await.map_err(|e| {
                        error!("Failed to lookup ecdsa aux info material: {e}");
                        RequestError::Lookup
                    })?;
                    elements.ecdsa_aux_info = Some(material);
                }
            };
        }
        Ok(elements)
    }
}

/// An error to satisfy an elements request.
#[derive(Debug, thiserror::Error)]
pub enum RequestError {
    /// Share lookup failed.
    #[error("share lookup failed")]
    Lookup,
}

struct Metrics {
    request_elements_duration: MaybeMetric<Histogram<Duration>>,
}

impl Default for Metrics {
    fn default() -> Self {
        let request_elements_duration = Histogram::new(
            "request_preprocessing_duration_seconds",
            "Duration of the processing of a request for preprocessing elements",
            &[],
            TimingBuckets::sub_second(),
        )
        .into();
        Self { request_elements_duration }
    }
}

impl Metrics {
    fn request_elements_timer(&self) -> ScopedTimer<impl SingleHistogramMetric<Duration>> {
        self.request_elements_duration.with_labels([]).into_timer()
    }
}
