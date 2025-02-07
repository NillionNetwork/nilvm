//! The values gRPC API.

use super::{extract_values, InvalidReceiptType};
use crate::{
    controllers::TraceRequest,
    services::{
        receipts::ReceiptsService,
        time::TimeService,
        user_values::{UserValuesAccessReason, UserValuesOperationError, UserValuesService},
    },
    storage::models::user_values::UserValuesRecord,
};
use async_trait::async_trait;
use chrono::{DateTime, Days, Utc};
use encoding::codec::MessageCodec;
use grpc_channel::auth::AuthenticateRequest;
use math_lib::modular::EncodedModulo;
use nada_value::protobuf::nada_values_from_protobuf;
use node_api::{
    auth::rust::UserId,
    membership::rust::Prime,
    payments::rust::{OperationMetadata, Receipt, RetrieveValues},
    permissions::rust::Permissions,
    values::{
        proto,
        rust::{
            DeleteValuesRequest, NamedValue, RetrieveValuesRequest, RetrieveValuesResponse, StoreValuesRequest,
            StoreValuesResponse,
        },
    },
    ConvertProto, TryIntoRust,
};
use std::sync::Arc;
use tonic::{Request, Response, Status};
use tracing::{error, info, instrument};
use uuid::Uuid;

const DEFAULT_TTL_DAYS: u32 = 365 * 10;

pub(crate) struct ValuesApiServices {
    pub(crate) user_values: Arc<dyn UserValuesService>,
    pub(crate) receipts: Arc<dyn ReceiptsService>,
    pub(crate) time: Arc<dyn TimeService>,
}

pub(crate) struct ValuesApi {
    services: ValuesApiServices,
    prime: Prime,
    modulo: EncodedModulo,
}

impl ValuesApi {
    pub(crate) fn new(services: ValuesApiServices, prime: Prime) -> Self {
        let modulo = match &prime {
            Prime::Safe64Bits => EncodedModulo::U64SafePrime,
            Prime::Safe128Bits => EncodedModulo::U128SafePrime,
            Prime::Safe256Bits => EncodedModulo::U256SafePrime,
        };
        Self { services, prime, modulo }
    }

    async fn do_store_values(
        &self,
        record: PartialUserValuesRecord,
        permissions: Option<Permissions>,
        identifier: Vec<u8>,
    ) -> tonic::Result<Uuid> {
        let permissions =
            permissions.ok_or_else(|| Status::invalid_argument("'permissions' is required when storing values"))?;
        let values_id = Uuid::from_slice(&identifier).map_err(|_| Status::internal("invalid uuid"))?;
        let PartialUserValuesRecord { values, expires_at } = record;
        let record = UserValuesRecord { values, permissions, expires_at, prime: self.prime.clone() };

        info!("Storing values with id {values_id}");
        self.services.user_values.create_if_not_exists(values_id, record).await?;
        Ok(values_id)
    }

    async fn do_update_values(
        &self,
        record: PartialUserValuesRecord,
        permissions: Option<Permissions>,
        identifier: Vec<u8>,
        user: &UserId,
    ) -> tonic::Result<Uuid> {
        let values_id = Uuid::from_slice(&identifier).map_err(|_| Status::internal("invalid uuid"))?;
        // look up to get existing permissions + validate access control
        let existing_record =
            self.services.user_values.find(values_id, user, &UserValuesAccessReason::UpdateUserValues).await?;
        // if we're given permissions, use those. otherwise fall back to keeping the ones the
        // record already had
        let permissions = match permissions {
            Some(permissions) if existing_record.permissions.update.contains(user) => permissions,
            Some(_) => return Err(Status::permission_denied("user not allowed to update permissions")),
            None => existing_record.permissions,
        };
        let PartialUserValuesRecord { values, expires_at } = record;

        info!("Updating values with id {values_id}");
        let record = UserValuesRecord { values, permissions, expires_at, prime: self.prime.clone() };
        self.services.user_values.upsert(values_id, record).await?;
        Ok(values_id)
    }
}

#[async_trait]
impl proto::values_server::Values for ValuesApi {
    #[instrument(name = "api.values.delete_values", skip_all, fields(user_id = request.trace_user_id()))]
    async fn delete_values(&self, request: Request<proto::delete::DeleteValuesRequest>) -> tonic::Result<Response<()>> {
        let user_id = request.user_id()?;
        let request: DeleteValuesRequest = request.into_inner().try_into_rust()?;
        let values_id = Uuid::from_slice(&request.values_id).map_err(|_| Status::invalid_argument("invalid id"))?;

        info!("Deleting values with id {values_id}");
        self.services.user_values.delete(values_id, &user_id).await?;
        Ok(Response::new(()))
    }

    #[instrument(name = "api.values.retrieve_values", skip_all, fields(user_id = request.trace_user_id()))]
    async fn retrieve_values(
        &self,
        request: Request<proto::retrieve::RetrieveValuesRequest>,
    ) -> tonic::Result<Response<proto::retrieve::RetrieveValuesResponse>> {
        let user_id = request.user_id()?;
        let RetrieveValuesRequest { signed_receipt } = request.into_inner().try_into_rust()?;
        let receipt = self.services.receipts.verify_payment_receipt(signed_receipt).await?;
        let OperationMetadata::RetrieveValues(RetrieveValues { values_id }) = receipt.metadata else {
            return Err(InvalidReceiptType("retrieve values").into());
        };
        let id = Uuid::from_slice(&values_id).map_err(|_| Status::invalid_argument("invalid values identifier"))?;

        info!("Looking up values with id {id}");
        let values =
            self.services.user_values.find(id, &user_id, &UserValuesAccessReason::RetrieveUserValues).await?.values;
        // We temporarily return both protobuf and bincode encoded values until all clients are
        // migrated to use protobuf.
        let nada_values = nada_values_from_protobuf(values.clone(), &self.modulo).map_err(|e| {
            error!("Value {id} is corrupted: {e}");
            Status::internal("values are corrupted")
        })?;
        let bincode_values = MessageCodec.encode(&nada_values).map_err(|_| Status::internal("malformed values"))?;
        Ok(Response::new(RetrieveValuesResponse { values, bincode_values }.into_proto()))
    }

    #[instrument(name = "api.values.store_values", skip_all, fields(user_id = request.trace_user_id()))]
    async fn store_values(
        &self,
        request: Request<proto::store::StoreValuesRequest>,
    ) -> tonic::Result<Response<proto::store::StoreValuesResponse>> {
        let user_id = request.user_id()?;
        let StoreValuesRequest { values, bincode_values, permissions, signed_receipt, update_identifier } =
            request.into_inner().try_into_rust()?;
        let Receipt { identifier, metadata, .. } =
            self.services.receipts.verify_payment_receipt(signed_receipt).await?;
        let OperationMetadata::StoreValues(metadata) = metadata else {
            return Err(InvalidReceiptType("store values").into());
        };
        let values = extract_values(values, &bincode_values, &self.modulo)?;
        if values.is_empty() {
            return Err(Status::invalid_argument("no values provided"));
        }
        let ttl_days = metadata.ttl_days.unwrap_or(DEFAULT_TTL_DAYS);
        let expires_at = self
            .services
            .time
            .current_time()
            .checked_add_days(Days::new(ttl_days as u64))
            .ok_or_else(|| Status::invalid_argument("ttl_days is too high"))?;
        let record = PartialUserValuesRecord { values, expires_at };
        let values_id = match update_identifier {
            Some(identifier) => self.do_update_values(record, permissions, identifier, &user_id).await?,
            None => self.do_store_values(record, permissions, identifier).await?,
        };
        Ok(Response::new(StoreValuesResponse { values_id: values_id.into() }))
    }
}

impl From<UserValuesOperationError> for Status {
    fn from(e: UserValuesOperationError) -> Self {
        match e {
            UserValuesOperationError::Unauthorized => Status::permission_denied(e.to_string()),
            UserValuesOperationError::NotFound => Status::not_found(e.to_string()),
            UserValuesOperationError::AlreadyExists => Status::failed_precondition(e.to_string()),
            UserValuesOperationError::Expired => Status::not_found(e.to_string()),
            UserValuesOperationError::Internal(e) => {
                error!("User values operation failed: {e}");
                Status::internal("internal error")
            }
        }
    }
}

struct PartialUserValuesRecord {
    values: Vec<NamedValue>,
    expires_at: DateTime<Utc>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        controllers::tests::{empty_signed_receipt, MakeAuthenticated, ReceiptBuilder},
        services::{receipts::MockReceiptsService, time::MockTimeService, user_values::MockUserValuesService},
        storage::models::user_values::UserValuesRecord,
    };
    use chrono::Utc;
    use math_lib::modular::{EncodedModularNumber, EncodedModulo};
    use mockall::predicate::eq;
    use nada_value::{
        encrypted::{Encoded, Encrypted},
        protobuf::nada_values_to_protobuf,
        NadaValue,
    };
    use node_api::{
        payments::rust::{Receipt, RetrieveValues, StoreValues},
        permissions::rust::ComputePermission,
    };
    use proto::values_server::Values;
    use std::{collections::HashMap, time::Duration};
    use tonic::Code;

    #[derive(Default)]
    struct ServiceBuilder {
        user_values: MockUserValuesService,
        receipts: MockReceiptsService,
        time: MockTimeService,
    }

    impl ServiceBuilder {
        fn build(self) -> ValuesApi {
            ValuesApi::new(
                ValuesApiServices {
                    user_values: Arc::new(self.user_values),
                    receipts: Arc::new(self.receipts),
                    time: Arc::new(self.time),
                },
                Prime::Safe64Bits,
            )
        }
    }

    fn make_receipt(nonce: &[u8], ttl_days: u64) -> Receipt {
        Receipt {
            identifier: nonce.to_vec(),
            metadata: OperationMetadata::StoreValues(StoreValues {
                secret_shared_count: 0,
                public_values_count: 0,
                ttl_days: Some(ttl_days as u32),
                payload_size: 0,
                ecdsa_private_key_shares_count: 0,
                ecdsa_signature_shares_count: 0,
            }),
            expires_at: Utc::now(),
        }
    }

    fn empty_permissions() -> Permissions {
        Permissions {
            owner: UserId::from_bytes("bob"),
            retrieve: Default::default(),
            update: Default::default(),
            delete: Default::default(),
            compute: Default::default(),
        }
    }

    #[tokio::test]
    async fn delete_values() {
        let id = Uuid::new_v4();
        let user_id = UserId::from_bytes("bob");
        let mut builder = ServiceBuilder::default();
        builder.user_values.expect_delete().with(eq(id), eq(user_id)).return_once(move |_, _| Ok(()));

        let request = Request::new(proto::delete::DeleteValuesRequest { values_id: id.into_bytes().to_vec() })
            .authenticated(user_id);
        let api = builder.build();
        api.delete_values(request).await.expect("request failed");
    }

    #[tokio::test]
    async fn retrieve_values() {
        let id = Uuid::new_v4();
        let user_id = UserId::from_bytes("bob");
        let mut builder = ServiceBuilder::default();
        let values = UserValuesRecord {
            values: Default::default(),
            permissions: empty_permissions(),
            expires_at: Utc::now(),
            prime: Prime::Safe64Bits,
        };
        let receipt = ReceiptBuilder::new(RetrieveValues { values_id: id.into_bytes().to_vec() }).build();
        builder
            .user_values
            .expect_find()
            .with(eq(id), eq(user_id), eq(UserValuesAccessReason::RetrieveUserValues))
            .return_once(move |_, _, _| Ok(values));
        builder.receipts.expect_verify_payment_receipt().return_once(move |_| Ok(receipt));

        let request = Request::new(RetrieveValuesRequest { signed_receipt: empty_signed_receipt() }.into_proto())
            .authenticated(user_id);
        let api = builder.build();
        api.retrieve_values(request).await.expect("request failed");
    }

    #[tokio::test]
    async fn retrieve_non_existent() {
        let id = Uuid::new_v4();
        let user_id = UserId::from_bytes("bob");
        let mut builder = ServiceBuilder::default();
        let receipt = ReceiptBuilder::new(RetrieveValues { values_id: id.into_bytes().to_vec() }).build();
        builder.user_values.expect_find().return_once(move |_, _, _| Err(UserValuesOperationError::NotFound));
        builder.receipts.expect_verify_payment_receipt().return_once(move |_| Ok(receipt));

        let request = Request::new(RetrieveValuesRequest { signed_receipt: empty_signed_receipt() }.into_proto())
            .authenticated(user_id);
        let api = builder.build();
        let error = api.retrieve_values(request).await.expect_err("request failed");
        assert_eq!(error.code(), Code::NotFound);
    }

    #[tokio::test]
    async fn store_values() {
        let nonce = vec![0; 16];

        let ttl_days = 3;
        let current_time = Utc::now();
        let expiration = current_time + Duration::from_secs(60 * 60 * 24 * ttl_days);
        let user_id = UserId::from_bytes("bob");
        let mut builder = ServiceBuilder::default();
        builder.time.expect_current_time().return_once(move || current_time);
        let permissions = Permissions {
            owner: UserId::from_bytes("bob"),
            retrieve: [UserId::from_bytes("r")].into(),
            update: [UserId::from_bytes("u")].into(),
            delete: [UserId::from_bytes("d")].into(),
            compute: [(UserId::from_bytes("c"), ComputePermission { program_ids: ["p".into()].into() })].into(),
        };
        let values: HashMap<String, NadaValue<Encrypted<Encoded>>> = [(
            "foo".into(),
            NadaValue::new_integer(EncodedModularNumber::new_unchecked(vec![1], EncodedModulo::U64SafePrime)),
        )]
        .into();
        let record = UserValuesRecord {
            values: nada_values_to_protobuf(values).unwrap(),
            permissions: permissions.clone(),
            expires_at: expiration,
            prime: Prime::Safe64Bits,
        };
        let receipt = make_receipt(&nonce, ttl_days);
        builder
            .user_values
            .expect_create_if_not_exists()
            .with(eq(Uuid::nil()), eq(record.clone()))
            .return_once(move |_, _| Ok(()));
        builder.receipts.expect_verify_payment_receipt().return_once(move |_| Ok(receipt));

        let request = Request::new(
            StoreValuesRequest {
                signed_receipt: empty_signed_receipt(),
                values: record.values,
                bincode_values: Vec::new(),
                permissions: Some(permissions),
                update_identifier: None,
            }
            .into_proto(),
        )
        .authenticated(user_id);
        let api = builder.build();
        let response = api.store_values(request).await.expect("request failed").into_inner();
        assert_eq!(response.values_id, vec![0; 16]);
    }

    #[tokio::test]
    async fn update_values() {
        let identifier = vec![1; 16];
        let identifier_uuid = Uuid::from_slice(&identifier).unwrap();
        let nonce = vec![0; 16];

        let ttl_days = 3;
        let current_time = Utc::now();
        let expiration = current_time + Duration::from_secs(60 * 60 * 24 * ttl_days);
        let user_id = UserId::from_bytes("bob");
        let mut builder = ServiceBuilder::default();
        builder.time.expect_current_time().return_once(move || current_time);

        let nada_values: HashMap<String, NadaValue<Encrypted<Encoded>>> = HashMap::from([(
            "foo".to_string(),
            NadaValue::new_integer(EncodedModularNumber::new_unchecked(vec![1], EncodedModulo::U64SafePrime)),
        )]);

        let permissions = empty_permissions();
        // what was already there
        let stored_record = UserValuesRecord {
            values: Default::default(),
            permissions: permissions.clone(),
            expires_at: Utc::now(),
            prime: Prime::Safe64Bits,
        };
        // what we update it with
        let updated_record = UserValuesRecord {
            values: nada_values_to_protobuf(nada_values.clone()).unwrap(),
            permissions,
            expires_at: expiration,
            prime: Prime::Safe64Bits,
        };

        let receipt = make_receipt(&nonce, ttl_days);
        builder
            .user_values
            .expect_upsert()
            .with(eq(identifier_uuid), eq(updated_record.clone()))
            .return_once(move |_, _| Ok(()));
        builder
            .user_values
            .expect_find()
            .with(eq(identifier_uuid), eq(user_id.clone()), eq(UserValuesAccessReason::UpdateUserValues))
            .return_once(move |_, _, _| Ok(stored_record));
        builder.receipts.expect_verify_payment_receipt().return_once(move |_| Ok(receipt));

        let request = Request::new(
            StoreValuesRequest {
                signed_receipt: empty_signed_receipt(),
                values: updated_record.values,
                bincode_values: Vec::new(),
                permissions: None,
                update_identifier: Some(identifier.clone()),
            }
            .into_proto(),
        )
        .authenticated(user_id);
        let api = builder.build();
        let response = api.store_values(request).await.expect("request failed").into_inner();
        assert_eq!(response.values_id, identifier);
    }
}
