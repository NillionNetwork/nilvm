//! Invoke compute operation.

use super::{compute_values_size, BuildError, CollapseResult, InvokeError, PaidOperation, PaidVmOperation};
use crate::{grpc::ComputeClient, retry::Retrier, vm::VmClient, UserId};
use nada_value::protobuf::nada_values_to_protobuf;
use nillion_client_core::values::{Clear, CleartextValues, EncryptedValues, NadaValue, PartyShares};
use node_api::{
    compute::rust::{InputPartyBinding, InvokeComputeRequest, OutputPartyBinding},
    payments::rust::{InvokeCompute, PriceQuoteRequest, SignedReceipt},
};
use std::collections::HashSet;
use tonic::async_trait;
use uuid::Uuid;

/// An invoke compute operation.
pub struct InvokeComputeOperation {
    value_ids: Vec<Vec<u8>>,
    values: PartyShares<EncryptedValues>,
    input_bindings: Vec<InputPartyBinding>,
    output_bindings: Vec<OutputPartyBinding>,
    operation: InvokeCompute,
}

#[async_trait]
impl PaidVmOperation for InvokeComputeOperation {
    type Output = Uuid;

    const NAME: &str = "invoke-compute";

    fn price_quote_request(&self) -> PriceQuoteRequest {
        PriceQuoteRequest::InvokeCompute(self.operation.clone())
    }

    async fn invoke(mut self, vm: &VmClient, signed_receipt: SignedReceipt) -> Result<Self::Output, InvokeError> {
        let mut retrier = Retrier::default();
        for (party, clients) in &vm.clients {
            let values =
                self.values.remove(party).ok_or_else(|| InvokeError(format!("shares for party {party} not found")))?;
            let values =
                nada_values_to_protobuf(values).map_err(|e| InvokeError(format!("failed to encode values: {e}")))?;
            let request = InvokeComputeRequest {
                signed_receipt: signed_receipt.clone(),
                value_ids: self.value_ids.clone(),
                values,
                input_bindings: self.input_bindings.clone(),
                output_bindings: self.output_bindings.clone(),
            };
            retrier.add_request(party.clone(), &clients.compute, request);
        }
        let results = retrier.invoke(ComputeClient::invoke_compute).await;
        let compute_id = results.collapse(|r| r.compute_id)?;
        Uuid::from_slice(&compute_id).map_err(|_| InvokeError("malformed compute_id returned".into()))
    }
}

/// A builder for an invoke compute operation.
#[must_use]
pub struct InvokeComputeOperationBuilder<'a> {
    vm: &'a VmClient,
    program_id: Option<String>,
    value_ids: Vec<Vec<u8>>,
    values: CleartextValues,
    input_bindings: Vec<InputPartyBinding>,
    output_bindings: Vec<OutputPartyBinding>,
}

impl<'a> InvokeComputeOperationBuilder<'a> {
    pub(crate) fn new(vm: &'a VmClient) -> Self {
        Self {
            vm,
            program_id: Default::default(),
            value_ids: Default::default(),
            values: Default::default(),
            input_bindings: Default::default(),
            output_bindings: Default::default(),
        }
    }

    /// Set the program id to be used.
    pub fn program_id<S: Into<String>>(mut self, id: S) -> Self {
        self.program_id = Some(id.into());
        self
    }

    /// Adds the identifier for an already stored value.
    pub fn add_value_id(mut self, id: Uuid) -> Self {
        self.value_ids.push(id.into());
        self
    }

    /// Adds the identifiers for an stored values.
    pub fn add_value_ids<I>(mut self, ids: I) -> Self
    where
        I: IntoIterator<Item = Uuid>,
    {
        let ids = ids.into_iter().map(Into::into);
        self.value_ids.extend(ids);
        self
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

    /// Bind an input party in the program to a user id in the network.
    pub fn bind_input_party<N>(mut self, party_name: N, user: UserId) -> Self
    where
        N: Into<String>,
    {
        let party_name = party_name.into();
        self.input_bindings.push(InputPartyBinding { party_name, user });
        self
    }

    /// Bind an input party in the program to a user id in the network.
    pub fn bind_output_party<N, I>(mut self, party_name: N, users: I) -> Self
    where
        N: Into<String>,
        I: IntoIterator<Item = UserId>,
    {
        let party_name = party_name.into();
        let users = users.into_iter().collect();
        self.output_bindings.push(OutputPartyBinding { party_name, users });
        self
    }

    /// Build the operation.
    pub fn build(mut self) -> Result<PaidOperation<'a, InvokeComputeOperation>, BuildError> {
        let program_id = self.program_id.take().ok_or_else(|| BuildError("'program_id' not set".into()))?;
        if Self::contains_duplicates(self.input_bindings.iter().map(|i| &i.party_name)) {
            return Err(BuildError("input party bindings must be unique".into()));
        }
        if Self::contains_duplicates(self.output_bindings.iter().map(|i| &i.party_name)) {
            return Err(BuildError("output party bindings must be unique".into()));
        }
        let values = self.vm.masker.mask(self.values).map_err(|e| BuildError(format!("failed to mask values: {e}")))?;
        let values_payload_size = compute_values_size(&values)?;
        let operation = InvokeCompute { program_id, values_payload_size };
        let operation = InvokeComputeOperation {
            value_ids: self.value_ids,
            values,
            input_bindings: self.input_bindings,
            output_bindings: self.output_bindings,
            operation,
        };
        Ok(PaidOperation::new(operation, self.vm))
    }

    fn contains_duplicates<'b, I>(iter: I) -> bool
    where
        I: Iterator<Item = &'b String> + ExactSizeIterator,
    {
        let total = iter.len();
        let names: HashSet<_> = iter.collect();
        names.len() != total
    }
}
