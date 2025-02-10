use crate::{
    controllers::{InvalidReceiptType, TraceRequest},
    services::{
        receipts::ReceiptsService,
        user_values::{UserValuesAccessReason, UserValuesService},
    },
};
use async_trait::async_trait;
use grpc_channel::auth::AuthenticateRequest;
use node_api::{
    payments::rust::{OperationMetadata, OverwritePermissions, RetrievePermissions, UpdatePermissions},
    permissions::{
        proto,
        rust::{OverwritePermissionsRequest, RetrievePermissionsRequest, UpdatePermissionsRequest},
    },
    ConvertProto, TryIntoRust,
};
use std::sync::Arc;
use tonic::{Request, Response, Status};
use tracing::{info, instrument};
use uuid::Uuid;

pub(crate) struct PermissionsApiServices {
    pub(crate) receipts: Arc<dyn ReceiptsService>,
    pub(crate) user_values: Arc<dyn UserValuesService>,
}

pub(crate) struct PermissionsApi {
    services: PermissionsApiServices,
}

impl PermissionsApi {
    pub(crate) fn new(services: PermissionsApiServices) -> Self {
        Self { services }
    }
}

#[async_trait]
impl proto::permissions_server::Permissions for PermissionsApi {
    #[instrument(name = "api.permissions.retrieve_permissions", skip_all, fields(user_id = request.trace_user_id()))]
    async fn retrieve_permissions(
        &self,
        request: Request<proto::retrieve::RetrievePermissionsRequest>,
    ) -> tonic::Result<Response<proto::permissions::Permissions>> {
        let user_id = request.user_id()?;
        let request: RetrievePermissionsRequest = request.into_inner().try_into_rust()?;
        let receipt = self.services.receipts.verify_payment_receipt(request.signed_receipt).await?;
        let OperationMetadata::RetrievePermissions(RetrievePermissions { values_id }) = receipt.metadata else {
            return Err(InvalidReceiptType("retrieve permissions").into());
        };

        let values_id =
            Uuid::from_slice(&values_id).map_err(|_| Status::invalid_argument("invalid values identifier"))?;
        info!("Retrieving permissions for values id {values_id}");
        let values =
            self.services.user_values.find(values_id, &user_id, &UserValuesAccessReason::RetrievePermissions).await?;
        Ok(Response::new(values.permissions.into_proto()))
    }

    #[instrument(name = "api.permissions.overwrite_permissions", skip_all, fields(user_id = request.trace_user_id()))]
    async fn overwrite_permissions(
        &self,
        request: Request<proto::overwrite::OverwritePermissionsRequest>,
    ) -> tonic::Result<Response<()>> {
        let user_id = request.user_id()?;
        let request: OverwritePermissionsRequest = request.into_inner().try_into_rust()?;
        let receipt = self.services.receipts.verify_payment_receipt(request.signed_receipt).await?;
        let OperationMetadata::OverwritePermissions(OverwritePermissions { values_id }) = receipt.metadata else {
            return Err(InvalidReceiptType("overwrite permissions").into());
        };

        let values_id =
            Uuid::from_slice(&values_id).map_err(|_| Status::invalid_argument("invalid values identifier"))?;
        self.services.user_values.set_permissions(values_id, &user_id, request.permissions).await?;
        Ok(Response::new(()))
    }

    #[instrument(name = "api.permissions.update_permissions", skip_all, fields(user_id = request.trace_user_id()))]
    async fn update_permissions(
        &self,
        request: Request<proto::update::UpdatePermissionsRequest>,
    ) -> tonic::Result<Response<()>> {
        let user_id = request.user_id()?;
        let UpdatePermissionsRequest { signed_receipt, delta } = request.into_inner().try_into_rust()?;

        let receipt = self.services.receipts.verify_payment_receipt(signed_receipt).await?;
        let OperationMetadata::UpdatePermissions(UpdatePermissions { values_id }) = receipt.metadata else {
            return Err(InvalidReceiptType("update permissions").into());
        };

        let values_id =
            Uuid::from_slice(&values_id).map_err(|_| Status::invalid_argument("invalid values identifier"))?;
        self.services.user_values.apply_permissions_delta(values_id, &user_id, delta).await?;
        Ok(Response::new(()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        controllers::tests::{empty_signed_receipt, MakeAuthenticated, ReceiptBuilder},
        services::{receipts::MockReceiptsService, user_values::MockUserValuesService},
        storage::models::user_values::UserValuesRecord,
    };
    use chrono::Utc;
    use mockall::predicate::eq;
    use node_api::{
        auth::rust::UserId,
        membership::rust::Prime,
        permissions::rust::{ComputePermission, Permissions},
        ConvertProto,
    };
    use proto::permissions_server::Permissions as _;

    #[derive(Default)]
    struct ServiceBuilder {
        user_values: MockUserValuesService,
        receipts: MockReceiptsService,
    }

    impl ServiceBuilder {
        fn build(self) -> PermissionsApi {
            PermissionsApi::new(PermissionsApiServices {
                user_values: Arc::new(self.user_values),
                receipts: Arc::new(self.receipts),
            })
        }
    }

    #[tokio::test]
    async fn retrieve() {
        let permissions = Permissions {
            owner: UserId::from_bytes("bob"),
            retrieve: [UserId::from_bytes("r")].into(),
            update: [UserId::from_bytes("u")].into(),
            delete: [UserId::from_bytes("d")].into(),
            compute: [(UserId::from_bytes("c"), ComputePermission { program_ids: ["p".into()].into() })].into(),
        };
        let user_values = UserValuesRecord {
            values: Default::default(),
            permissions: permissions.clone(),
            expires_at: Utc::now(),
            prime: Prime::Safe64Bits,
        };
        let user_id = UserId::from_bytes("bob");
        let values_id = Uuid::new_v4();
        let mut builder = ServiceBuilder::default();
        builder
            .user_values
            .expect_find()
            .with(eq(values_id), eq(user_id.clone()), eq(UserValuesAccessReason::RetrievePermissions))
            .return_once(move |_, _, _| Ok(user_values));

        let receipt = ReceiptBuilder::new(RetrievePermissions { values_id: values_id.into_bytes().to_vec() }).build();
        builder.receipts.expect_verify_payment_receipt().return_once(move |_| Ok(receipt));

        let request = Request::new(RetrievePermissionsRequest { signed_receipt: empty_signed_receipt() }.into_proto())
            .authenticated(user_id);
        let returned_permissions: Permissions = builder
            .build()
            .retrieve_permissions(request)
            .await
            .expect("request failed")
            .into_inner()
            .try_into_rust()
            .unwrap();
        assert_eq!(returned_permissions, permissions);
    }
}
