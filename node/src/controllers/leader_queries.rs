//! The leader queries gRPC API.

use super::InvalidReceiptType;
use crate::{
    controllers::TraceRequest,
    services::{
        auxiliary_material::AuxiliaryMaterialMetadataService, offsets::ElementOffsetsService, receipts::ReceiptsService,
    },
    storage::repositories::offsets::PreprocessingOffsets,
    PreprocessingConfigExt,
};
use async_trait::async_trait;
use node_api::{
    leader_queries::{
        proto,
        rust::{PoolStatusRequest, PoolStatusResponse},
    },
    payments::rust::OperationMetadata,
    preprocessing::rust::{AuxiliaryMaterial, PreprocessingElement},
    ConvertProto, TryIntoRust,
};
use node_config::PreprocessingConfig;
use std::{collections::BTreeMap, sync::Arc};
use tonic::{Request, Response, Status};
use tracing::{error, instrument};

pub(crate) struct LeaderQueriesApiServices {
    pub(crate) receipts: Arc<dyn ReceiptsService>,
    pub(crate) offsets: Arc<dyn ElementOffsetsService>,
    pub(crate) auxiliary_material_metadata: Arc<dyn AuxiliaryMaterialMetadataService>,
    pub(crate) preprocessing_config: PreprocessingConfig,
}

pub(crate) struct LeaderQueriesApi {
    services: LeaderQueriesApiServices,
}

impl LeaderQueriesApi {
    pub(crate) fn new(services: LeaderQueriesApiServices) -> Self {
        Self { services }
    }

    fn prepare_pool_status_response(
        offsets: BTreeMap<PreprocessingElement, PreprocessingOffsets>,
        auxiliary_material_available: bool,
        preprocessing_config: &PreprocessingConfig,
    ) -> PoolStatusResponse {
        let mut output = BTreeMap::new();
        let mut preprocessing_active = false;
        for (element, offsets) in offsets {
            let element_config = preprocessing_config.element_config(&element);
            let threshold = element_config.generation_threshold;
            let available_count = offsets.available().count() as u64;
            if available_count < threshold {
                preprocessing_active = true;
            }
            output.insert(element, offsets.available());
        }
        PoolStatusResponse { offsets: output, auxiliary_material_available, preprocessing_active }
    }
}

#[async_trait]
impl proto::leader_queries_server::LeaderQueries for LeaderQueriesApi {
    #[instrument(name = "api.leader_queries.pool_status", skip_all, fields(user_id = request.trace_user_id()))]
    async fn pool_status(
        &self,
        request: Request<proto::pool_status::PoolStatusRequest>,
    ) -> tonic::Result<Response<proto::pool_status::PoolStatusResponse>> {
        let request: PoolStatusRequest = request.into_inner().try_into_rust()?;
        let receipt = self.services.receipts.verify_payment_receipt(request.signed_receipt).await?;
        if !matches!(receipt.metadata, OperationMetadata::PoolStatus) {
            return Err(InvalidReceiptType("pool status").into());
        }

        let offsets = self.services.offsets.all_offsets().await.map_err(|e| {
            error!("Failed to fetch offsets: {e}");
            Status::internal("internal error")
        })?;

        let aux_material =
            self.services.auxiliary_material_metadata.versions(&[AuxiliaryMaterial::Cggmp21AuxiliaryInfo]).await;
        let is_aux_material_available = match aux_material {
            Ok(rows) if !rows.is_empty() => true,
            Ok(_) | Err(_) => false,
        };

        let response =
            Self::prepare_pool_status_response(offsets, is_aux_material_available, &self.services.preprocessing_config);
        Ok(Response::new(response.into_proto()))
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::{
        controllers::tests::empty_signed_receipt,
        services::{
            auxiliary_material::MockAuxiliaryMaterialMetadataService, offsets::MockElementOffsetsService,
            receipts::MockReceiptsService,
        },
    };
    use chrono::Utc;
    use node_api::{
        payments::rust::{OperationMetadata, Receipt},
        preprocessing::proto::material::AuxiliaryMaterial::Cggmp21AuxiliaryInfo,
    };
    use proto::leader_queries_server::LeaderQueries;
    use std::collections::HashMap;

    #[derive(Default)]
    struct Services {
        receipts: MockReceiptsService,
        offsets: MockElementOffsetsService,
        auxiliary_material_metadata: MockAuxiliaryMaterialMetadataService,
        preprocessing_config: PreprocessingConfig,
    }

    impl From<Services> for LeaderQueriesApiServices {
        fn from(services: Services) -> Self {
            Self {
                receipts: Arc::new(services.receipts),
                offsets: Arc::new(services.offsets),
                auxiliary_material_metadata: Arc::new(services.auxiliary_material_metadata),
                preprocessing_config: services.preprocessing_config,
            }
        }
    }

    #[tokio::test]
    async fn pool_status() {
        let nonce = vec![1, 2, 3];
        let expiration = Utc::now();
        let receipt =
            Receipt { identifier: nonce.clone(), metadata: OperationMetadata::PoolStatus, expires_at: expiration };
        let offsets = BTreeMap::from([(
            PreprocessingElement::Compare,
            PreprocessingOffsets {
                element: PreprocessingElement::Compare,
                target: 150,
                latest: 100,
                committed: 50,
                next_batch_id: 2,
                deleted_offset: 0,
                delete_candidate_offset: 0,
            },
        )]);
        let signed_receipt = empty_signed_receipt();
        let mut services = Services::default();
        services.receipts.expect_verify_payment_receipt().return_once(move |_| Ok(receipt));
        services.offsets.expect_all_offsets().return_once(move || Ok(offsets));
        services
            .auxiliary_material_metadata
            .expect_versions()
            .with(mockall::predicate::eq(vec![Cggmp21AuxiliaryInfo]))
            .return_once(|_| {
                let mut response = HashMap::new();
                response.insert(Cggmp21AuxiliaryInfo, 0);
                Ok(response)
            });

        let api = LeaderQueriesApi::new(services.into());
        let request = PoolStatusRequest { signed_receipt }.into_proto();
        let offsets: PoolStatusResponse = api
            .pool_status(Request::new(request))
            .await
            .expect("request failed")
            .into_inner()
            .try_into_rust()
            .expect("invalid response");
        assert_eq!(
            offsets.offsets,
            BTreeMap::from([(node_api::preprocessing::rust::PreprocessingElement::Compare, 50..100)])
        );
        assert_eq!(offsets.preprocessing_active, false);
    }
}
