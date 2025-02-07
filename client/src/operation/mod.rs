//! VM operations.

use crate::{
    grpc::PaymentsClient,
    payments::TxHash,
    retry::Retrier,
    vm::{PaymentMode, VmClient},
};
use nada_value::protobuf::nada_values_to_protobuf;
use nillion_chain_client::transactions::TokenAmount;
use nillion_client_core::values::{EncryptedValues, PartyId, PartyShares};
use node_api::{
    errors::StatusExt,
    payments::rust::{PaymentReceiptRequest, PriceQuote, PriceQuoteRequest, QuoteFees, SignedQuote, SignedReceipt},
    values::rust::NamedValue,
    ConvertProto, Message,
};
use std::{any::type_name, fmt, time::Instant};
use tonic::{async_trait, Status};
use tracing::{info, instrument, warn};

pub mod add_funds;
pub mod delete_values;
pub mod invoke_compute;
pub mod overwrite_permissions;
pub mod pool_status;
pub mod retrieve_compute_results;
pub mod retrieve_permissions;
pub mod retrieve_values;
pub mod store_program;
pub mod store_values;
pub mod update_permissions;

const PAYMENT_RECEIPT_MAX_RETRIES: usize = 10;
const PRICE_QUOTE_MAX_RETRIES: usize = 10;

/// A paid operation in the NilVm.
///
/// This represents a paid operation in the network. Operations require being drive to completion
/// manually by either:
///
/// * Invoking [PaidOperation::invoke] which will under the hood get a price quote, pay for it using
///   the provided [NilChainPayer][crate::payments::NilChainPayer] instance, get the payment validated by the
///   cluster's leader, and finally invoke the operation.
/// * Invoking all of the steps in the previous point one by one, starting with
///   [PaidOperation::invoke].
pub struct PaidOperation<'a, O, S = InitialState> {
    operation: O,
    state: S,
    client: &'a VmClient,
}

impl<'a, O, S: fmt::Debug> fmt::Debug for PaidOperation<'a, O, S> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Get the type name without all the module qualifiers
        let operation_name = type_name::<O>();
        let operation_name = operation_name.rsplit_once("::").map(|(name, _)| name).unwrap_or(operation_name);
        f.debug_struct("PaidOperation").field("operation", &operation_name).field("state", &self.state).finish()
    }
}

impl<'a, O> PaidOperation<'a, O, InitialState> {
    pub(crate) fn new(operation: O, client: &'a VmClient) -> Self {
        Self { operation, state: InitialState, client }
    }
}

impl<'a, O> PaidOperation<'a, O, InitialState>
where
    O: PaidVmOperation,
{
    /// Get a price quote for this operation.
    ///
    /// After getting a price quote, [PaidOperation::pay] must be invoked to pay for this operation
    /// and drive its execution forward.
    #[instrument("operation.quote", skip_all, fields(operation = O::NAME))]
    pub async fn quote(self) -> Result<PaidOperation<'a, O, QuotedState>, QuoteError> {
        let Self { operation, client, .. } = self;
        let request = operation.price_quote_request();
        let start_time = Instant::now();
        let mut retrier = Retrier::default().with_max_retries(PRICE_QUOTE_MAX_RETRIES);
        let leader_party = PartyId::from(Vec::from(client.cluster.leader.identity.clone()));
        retrier.add_request(leader_party, &client.payments, request);

        let signed_quote = retrier
            .invoke_single(PaymentsClient::price_quote)
            .await
            .map_err(|e| QuoteError(format!("failed to get price quote: {e}")))?;
        info!("Operation quoted in {:?}", start_time.elapsed());
        let quote = PriceQuote::try_decode(&signed_quote.quote)
            .map_err(|e| QuoteError(format!("invalid price quote received: {e}")))?;
        let state = QuotedState { signed_quote, quote, payment_mode: client.payment_mode.clone() };
        Ok(PaidOperation { operation, state, client })
    }

    /// Invoke the operation.
    ///
    /// This is the equivalent of calling:
    ///
    /// ```ignore
    /// let output = operation
    ///   .quote().await?
    ///   .pay().await?
    ///   .validate().await?
    ///   .invoke().await?;
    /// ```
    pub async fn invoke(self) -> Result<O::Output, InitialStateInvokeError> {
        let output = self.quote().await?.pay().await?.validate().await?.invoke().await?;
        Ok(output)
    }
}

impl<'a, O> PaidOperation<'a, O, QuotedState>
where
    O: PaidVmOperation,
{
    /// Pay for this operation.
    ///
    /// This will use the [NilChainPayer][crate::payments::NilChainPayer] instance provided during the client's
    /// construction to perform the payment.
    ///
    /// After paying, [PaidOperation::validate] must be called to validate the operation against
    /// the cluster's leader and drive this operation's execution forward.
    #[instrument("operation.pay", skip_all, fields(operation = O::NAME))]
    pub async fn pay(self) -> Result<PaidOperation<'a, O, PaidState>, PaymentError> {
        let Self { operation, client, state } = self;
        let state = match state.payment_mode {
            PaymentMode::FromBalance => {
                PaidState { signed_quote: state.signed_quote, quote: state.quote, tx_hash: None }
            }
            PaymentMode::PayPerOperation => {
                let start_time = Instant::now();
                let tx_hash = client
                    .nilchain_payer
                    .submit_payment(state.quote.fees.total, state.quote.nonce.clone())
                    .await
                    .map_err(|e| PaymentError(e.to_string()))?;
                info!("Payment of {} made in {:?}", TokenAmount::Unil(state.quote.fees.total), start_time.elapsed());
                PaidState { signed_quote: state.signed_quote, quote: state.quote, tx_hash: Some(tx_hash) }
            }
        };
        Ok(PaidOperation { operation, state, client })
    }

    /// Get the fees that we were quoted for this operation.
    pub fn fees(&self) -> &QuoteFees {
        &self.state.quote.fees
    }

    /// Get the nonce for the underlying quote.
    pub fn nonce(&self) -> &[u8] {
        &self.state.quote.nonce
    }
}

impl<'a, O> PaidOperation<'a, O, PaidState>
where
    O: PaidVmOperation,
{
    /// Validate this operation's payment against the cluster's leader.
    ///
    /// After validation, [PaidOperation::invoke] must be called to drive this operation to
    /// completion.
    #[instrument("operation.validate", skip_all, fields(operation = O::NAME))]
    pub async fn validate(self) -> Result<PaidOperation<'a, O, ValidatedState>, ReceiptError> {
        let Self { operation, client, state } = self;
        let request =
            PaymentReceiptRequest { signed_quote: state.signed_quote.clone(), tx_hash: state.tx_hash.map(|h| h.0) };
        let mut retrier = Retrier::default().with_max_retries(PAYMENT_RECEIPT_MAX_RETRIES);
        let leader_party = PartyId::from(Vec::from(client.cluster.leader.identity.clone()));
        retrier.add_request(leader_party, &client.payments, request);

        let start_time = Instant::now();
        let result = retrier.invoke_single(PaymentsClient::payment_receipt).await;
        let signed_receipt = match result {
            Ok(receipt) => receipt,
            Err(e) if is_balance_error(&e) => {
                warn!("Not enough funds to perform operation, making a one time payment");
                let operation = PaidOperation {
                    operation,
                    client,
                    state: QuotedState {
                        signed_quote: state.signed_quote,
                        quote: state.quote,
                        payment_mode: PaymentMode::PayPerOperation,
                    },
                };
                let fut = operation
                    .pay()
                    .await
                    .map_err(|e| ReceiptError(format!("failed to pay when falling back to one time payment: {e}")))?
                    .validate();
                // box this so we don't have a recursive future type
                let fut = Box::pin(fut);
                return fut.await;
            }
            Err(e) => return Err(ReceiptError(format!("failed to get payment receipt: {e}"))),
        };
        let state = ValidatedState { signed_receipt };
        info!("Operation validated in {:?}", start_time.elapsed());
        Ok(PaidOperation { operation, state, client })
    }
}

impl<'a, O> PaidOperation<'a, O, ValidatedState>
where
    O: PaidVmOperation,
{
    /// Invoke this operation, driving it to completion.
    ///
    /// This is the final step in the chain of actions that are required to get the operation to be
    /// ran against the network.
    #[instrument("operation.invoke", skip_all, fields(operation = O::NAME))]
    pub async fn invoke(self) -> Result<O::Output, InvokeError> {
        let Self { operation, client, state } = self;
        let start_time = Instant::now();
        let output = operation.invoke(client, state.signed_receipt).await?;
        info!("Operation invoked in {:?}", start_time.elapsed());
        Ok(output)
    }
}

/// A VM operation that's free and can be invoked without any payments.
///
/// The operation must be drive to completion by invoking [FreeOperation::invoke].
#[must_use]
pub struct FreeOperation<'a, O> {
    operation: O,
    client: &'a VmClient,
}

impl<'a, O> FreeOperation<'a, O>
where
    O: FreeVmOperation,
{
    pub(crate) fn new(operation: O, client: &'a VmClient) -> Self {
        Self { operation, client }
    }

    /// Invoke this operation, driving it to completion.
    #[instrument("operation.invoke", skip_all, fields(operation = O::NAME))]
    pub async fn invoke(self) -> Result<O::Output, InvokeError> {
        let Self { operation, client, .. } = self;
        let output = operation.invoke(client).await?;
        Ok(output)
    }
}

/// An error while building an operation.
#[derive(Debug, thiserror::Error)]
#[error("failed to build operation: {0}")]
pub struct BuildError(String);

/// An error while getting a price quote.
#[derive(Debug, thiserror::Error)]
#[error("failed to get price quote: {0}")]
pub struct QuoteError(String);

impl From<Status> for QuoteError {
    fn from(value: Status) -> Self {
        let code = value.code();
        let message = value.message();
        Self(format!("{code:?}: {message}"))
    }
}

/// An error while making a payment.
#[derive(Debug, thiserror::Error)]
#[error("failed to make payment: {0}")]
pub struct PaymentError(String);

/// An error while getting the receipt for an operation from the cluster's leader.
#[derive(Debug, thiserror::Error)]
#[error("failed to get receipt: {0}")]
pub struct ReceiptError(String);

impl From<Status> for ReceiptError {
    fn from(value: Status) -> Self {
        let code = value.code();
        let message = value.message();
        Self(format!("{code:?}: {message}"))
    }
}

/// An error during the invocation of an operation.
#[derive(Debug, thiserror::Error)]
#[error("failed to invoke operation: {0}")]
pub struct InvokeError(pub(crate) String);

impl From<Status> for InvokeError {
    fn from(value: Status) -> Self {
        let code = value.code();
        let message = value.message();
        Self(format!("{code:?}: {message}"))
    }
}

/// An error when calling `invoke` on an operation.
#[derive(Debug, thiserror::Error)]
#[error("{0}")]
pub struct InitialStateInvokeError(String);

impl From<QuoteError> for InitialStateInvokeError {
    fn from(error: QuoteError) -> Self {
        Self(error.to_string())
    }
}

impl From<PaymentError> for InitialStateInvokeError {
    fn from(error: PaymentError) -> Self {
        Self(error.to_string())
    }
}

impl From<ReceiptError> for InitialStateInvokeError {
    fn from(error: ReceiptError) -> Self {
        Self(error.to_string())
    }
}

impl From<InvokeError> for InitialStateInvokeError {
    fn from(error: InvokeError) -> Self {
        Self(error.to_string())
    }
}

/// The initial state for an operation.
#[derive(Debug)]
pub struct InitialState;

/// The state of an operation for which we've got a price quote.
#[derive(Debug)]
pub struct QuotedState {
    signed_quote: SignedQuote,
    quote: PriceQuote,
    payment_mode: PaymentMode,
}

/// The state of an operation for which we've paid.
#[derive(Debug)]
pub struct PaidState {
    signed_quote: SignedQuote,
    quote: PriceQuote,
    tx_hash: Option<TxHash>,
}

/// The state of an operation that has been validated by the cluster's leader.
#[derive(Debug)]
pub struct ValidatedState {
    signed_receipt: SignedReceipt,
}

/// A concrete paid operation in the network.
///
/// This type is not meant to be used directly and should instead be used via [PaidOperation].
#[async_trait]
pub trait PaidVmOperation {
    /// The output of this operation.
    type Output;

    /// The name of this operation.
    const NAME: &str;

    /// Get the request to get a price quote for this operation.
    fn price_quote_request(&self) -> PriceQuoteRequest;

    /// Invoke this operation.
    async fn invoke(self, vm: &VmClient, receipt: SignedReceipt) -> Result<Self::Output, InvokeError>;
}

/// A concrete free operation in the network.
///
/// This type is not meant to be used directly and should instead be used via [FreeOperation].
#[async_trait]
pub trait FreeVmOperation {
    /// The output of this operation.
    type Output;

    /// The name of this operation.
    const NAME: &str;

    /// Invoke this operation.
    async fn invoke(self, vm: &VmClient) -> Result<Self::Output, InvokeError>;
}

// Allows collapsing a container of presumably equal elements into a single one.
//
// This allows turning a container like `Vec<Result<A, B>>` into a `A` as long as:
// * All entries in the `Vec` are `Ok(A)`.
// * All `A` in the `Vec` are equal.
pub(crate) trait CollapseResult: Sized {
    type Inner;

    fn collapse<F, T>(self, extract: F) -> Result<T, InvokeError>
    where
        F: Fn(Self::Inner) -> T,
        T: PartialEq;

    fn collapse_default(self) -> Result<Self::Inner, InvokeError>
    where
        Self::Inner: PartialEq,
    {
        self.collapse(|a| a)
    }
}

impl<V> CollapseResult for Vec<Result<V, Status>> {
    type Inner = V;

    fn collapse<F, T>(self, extract: F) -> Result<T, InvokeError>
    where
        F: Fn(Self::Inner) -> T,
        T: PartialEq,
    {
        let mut candidate: Option<T> = None;
        for result in self {
            let result = result?;
            let result = extract(result);
            match &candidate {
                Some(candidate) => {
                    if candidate != &result {
                        return Err(InvokeError("received different responses from nodes".into()));
                    }
                }
                None => {
                    candidate = Some(result);
                }
            };
        }
        candidate.ok_or_else(|| InvokeError("no response returned".into()))
    }
}

fn is_balance_error(e: &Status) -> bool {
    let Some(failure) = e.get_details_precondition_failure() else {
        return false;
    };
    for violation in failure.violations {
        if violation.r#type == "PAYMENT" && violation.subject == "BALANCE" {
            return true;
        }
    }
    false
}

pub(crate) fn compute_values_size(values: &PartyShares<EncryptedValues>) -> Result<u64, BuildError> {
    let first_value = values.values().next().ok_or_else(|| BuildError("no nodes".into()))?;
    let proto_values =
        nada_values_to_protobuf(first_value.clone()).map_err(|e| BuildError(format!("encoding failed: {e}")))?;
    let output: usize = proto_values
        .into_iter()
        .map(|NamedValue { name, value }| {
            let name_len = name.len();
            let value_len = value.map(|v| v.encoded_len()).unwrap_or(0);
            name_len.saturating_add(value_len)
        })
        .sum();
    Ok(output as u64)
}
