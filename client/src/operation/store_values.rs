//! Store values operation.

use super::{compute_values_size, BuildError, CollapseResult, InvokeError, PaidOperation, PaidVmOperation};
use crate::{grpc::ValuesClient, retry::Retrier, vm::VmClient, UserId};
use nada_value::protobuf::nada_values_to_protobuf;
use nillion_client_core::values::{Clear, CleartextValues, EncryptedValues, NadaValue, PartyShares};
use node_api::{
    payments::rust::{PriceQuoteRequest, SignedReceipt, StoreValues},
    permissions::rust::Permissions,
    values::rust::StoreValuesRequest,
};
use tonic::async_trait;
use uuid::Uuid;

/// A store values operation.
pub struct StoreValuesOperation {
    values: PartyShares<EncryptedValues>,
    permissions: Option<Permissions>,
    operation: StoreValues,
    update_identifier: Option<Vec<u8>>,
}

#[async_trait]
impl PaidVmOperation for StoreValuesOperation {
    type Output = Uuid;

    const NAME: &str = "store-values";

    fn price_quote_request(&self) -> PriceQuoteRequest {
        PriceQuoteRequest::StoreValues(self.operation)
    }

    async fn invoke(mut self, vm: &VmClient, receipt: SignedReceipt) -> Result<Self::Output, InvokeError> {
        let mut retrier = Retrier::default();
        for (party, clients) in vm.clients.iter() {
            let values =
                self.values.remove(party).ok_or_else(|| InvokeError(format!("shares for party {party} not found")))?;
            let values =
                nada_values_to_protobuf(values).map_err(|e| InvokeError(format!("failed to encode values: {e}")))?;
            let request = StoreValuesRequest {
                values,
                bincode_values: Vec::new(),
                permissions: self.permissions.clone(),
                signed_receipt: receipt.clone(),
                update_identifier: self.update_identifier.clone(),
            };
            retrier.add_request(party.clone(), &clients.values, request);
        }
        let results = retrier.invoke(ValuesClient::store_values).await;
        let values_id = results.collapse(|r| r.values_id)?;
        Uuid::from_slice(&values_id).map_err(|_| InvokeError("malformed values_id returned".into()))
    }
}

/// A builder for a store values operation.
///
/// See [PaidOperation] for more information.
#[must_use]
pub struct StoreValuesOperationBuilder<'a> {
    vm: &'a VmClient,
    values: CleartextValues,
    permissions: Option<Permissions>,
    update_identifier: Option<Vec<u8>>,
    ttl_days: Option<u32>,
}

impl<'a> StoreValuesOperationBuilder<'a> {
    pub(crate) fn new(vm: &'a VmClient) -> Self {
        Self { vm, values: Default::default(), permissions: None, update_identifier: None, ttl_days: None }
    }

    fn default_permissions(user: UserId) -> Permissions {
        Permissions {
            owner: user,
            retrieve: [user].into_iter().collect(),
            update: [user].into_iter().collect(),
            delete: [user].into_iter().collect(),
            compute: Default::default(),
        }
    }

    fn get_or_init_permissions(&mut self) -> &mut Permissions {
        self.permissions.get_or_insert_with(|| Self::default_permissions(self.vm.user_id))
    }

    /// Add a value with the given name.
    pub fn add_value<S: Into<String>>(mut self, name: S, value: NadaValue<Clear>) -> Self {
        self.values.insert(name.into(), value);
        self
    }

    /// Add a set of values.
    pub fn add_values<I, S>(mut self, values: I) -> Self
    where
        I: IntoIterator<Item = (S, NadaValue<Clear>)>,
        S: Into<String>,
    {
        for (name, value) in values {
            self.values.insert(name.into(), value);
        }
        self
    }

    /// Allow a user to retrieve these values.
    pub fn allow_retrieve(mut self, user: UserId) -> Self {
        self.get_or_init_permissions().retrieve.insert(user);
        self
    }

    /// Allow a user to delete these values.
    pub fn allow_delete(mut self, user: UserId) -> Self {
        self.get_or_init_permissions().delete.insert(user);
        self
    }

    /// Allow a user to update these values.
    pub fn allow_update(mut self, user: UserId) -> Self {
        self.get_or_init_permissions().update.insert(user);
        self
    }

    /// Allow a user to use these values on an execution of the given program id.
    pub fn allow_compute(mut self, user_id: UserId, program_id: String) -> Self {
        let permissions = self.get_or_init_permissions();
        permissions.compute.entry(user_id).or_default().program_ids.insert(program_id);
        self
    }

    /// Sets the update identifier.
    ///
    /// When this is provided, this will cause the values already stored in the network with the
    /// given identifier to be updated.
    pub fn update_identifier(mut self, identifier: Uuid) -> Self {
        self.update_identifier = Some(identifier.into());
        self
    }

    /// Set the expiration, in days, of the values being stored.
    pub fn ttl_days(mut self, ttl: u32) -> Self {
        self.ttl_days = Some(ttl);
        self
    }

    /// Build this operation.
    pub fn build(mut self) -> Result<PaidOperation<'a, StoreValuesOperation>, BuildError> {
        let ttl_days = self.ttl_days.take();
        if self.values.is_empty() {
            return Err(BuildError("'values' is empty".into()));
        }

        let classification = self.vm.masker.classify_values(&self.values);
        let values = self.vm.masker.mask(self.values).map_err(|e| BuildError(format!("failed to mask values: {e}")))?;
        let payload_size = compute_values_size(&values)?;
        let permissions = match (&self.update_identifier, self.permissions) {
            // update + permissions => override permissions
            (Some(_), Some(permissions)) => Some(permissions),
            // update + no permissions => keep existing
            (Some(_), None) => None,
            // store + permissions => set these permissiosn
            (None, Some(permissions)) => Some(permissions),
            // store + no permissions => use default for this user
            (None, None) => Some(Self::default_permissions(self.vm.user_id)),
        };

        let operation = StoreValuesOperation {
            values,
            permissions,
            operation: StoreValues {
                secret_shared_count: classification.shares,
                public_values_count: classification.public,
                ecdsa_private_key_shares_count: classification.ecdsa_private_key_shares,
                ecdsa_signature_shares_count: classification.ecdsa_signature_shares,
                ttl_days,
                payload_size,
            },
            update_identifier: self.update_identifier,
        };
        Ok(PaidOperation::new(operation, self.vm))
    }
}
