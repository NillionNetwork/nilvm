//! The payments gRPC API.

use super::compute::ComputeHandles;
use crate::{
    controllers::TraceRequest,
    services::payments::{AddFundsError, PaymentService, PaymentVerificationError, QuoteError},
};
use async_trait::async_trait;
use chrono::Days;
use grpc_channel::auth::AuthenticateRequest;
use node_api::{
    errors::{ErrorDetails, PreconditionViolation, StatusExt},
    payments::{
        proto::{self, balance::AddFundsRequest},
        rust::{AccountBalanceResponse, PaymentReceiptRequest, PriceQuote, PriceQuoteRequest},
    },
    ConvertProto, TryIntoRust,
};
use std::{ops::Add, sync::Arc};
use tonic::{Code, Request, Response, Status};
use tracing::{error, info, instrument};
use uuid::Uuid;

/// The services used by the payments API.
pub(crate) struct PaymentsApiServices {
    pub(crate) payments: Arc<dyn PaymentService>,
}

/// The payments API.
pub(crate) struct PaymentsApi {
    services: PaymentsApiServices,
    compute_handles: ComputeHandles,
    max_concurrent_computes: usize,
    balance_expiration: Days,
}

impl PaymentsApi {
    pub(crate) fn new(
        compute_handles: ComputeHandles,
        max_concurrent_computes: usize,
        services: PaymentsApiServices,
        balance_expiration: Days,
    ) -> Self {
        Self { services, compute_handles, max_concurrent_computes, balance_expiration }
    }

    async fn validate_max_computes(&self, quote: &PriceQuote) -> tonic::Result<()> {
        if let PriceQuoteRequest::InvokeCompute { .. } = &quote.request {
            let compute_count = self.compute_handles.lock().await.len();
            let max = self.max_concurrent_computes;
            if compute_count > max {
                info!(
                    "Rejecting compute request because number of concurrent computes exceeds maximum: {compute_count} > {max}"
                );
                return Err(Status::unavailable("too many compute operations running, try again later"));
            } else {
                info!("Allowing execution because we have {compute_count} <= {max} active computes");
            }
        }
        Ok(())
    }
}

#[async_trait]
impl proto::payments_server::Payments for PaymentsApi {
    #[instrument(name = "api.payments.price_quote", skip_all, fields(user_id = request.trace_user_id()))]
    async fn price_quote(
        &self,
        request: Request<proto::quote::PriceQuoteRequest>,
    ) -> tonic::Result<Response<proto::quote::SignedQuote>> {
        let request: PriceQuoteRequest = request.into_inner().try_into_rust()?;
        let quote = self.services.payments.generate_quote(request).await?;
        Ok(Response::new(quote.into_proto()))
    }

    #[instrument(name = "api.payments.payment_receipt", skip_all, fields(user_id = request.trace_user_id()))]
    async fn payment_receipt(
        &self,
        request: tonic::Request<proto::receipt::PaymentReceiptRequest>,
    ) -> tonic::Result<Response<proto::receipt::SignedReceipt>> {
        let user_id = request.user_id();
        let PaymentReceiptRequest { signed_quote, tx_hash } = request.into_inner().try_into_rust()?;
        let quote = self.services.payments.verify_decode_quote(signed_quote)?;
        self.validate_max_computes(&quote).await?;
        let metadata = match tx_hash {
            Some(tx_hash) => {
                info!("Verifying payment with tx hash {tx_hash:?}");
                self.services.payments.verify_payment(quote, tx_hash).await?
            }
            None => {
                info!("Trying to use user funds to deduct payment");
                self.services.payments.deduct_payment_from_balance(quote, &user_id?).await?
            }
        };

        let nonce = Uuid::new_v4().as_bytes().to_vec();
        let receipt = self
            .services
            .payments
            .generate_payment_receipt(nonce, metadata)
            .map_err(|_| Status::internal("signing receipt failed"))?;
        Ok(Response::new(receipt.into_proto()))
    }

    #[instrument(name = "api.payments.payments_config", skip_all, fields(user_id = _request.trace_user_id()))]
    async fn payments_config(
        &self,
        _request: tonic::Request<()>,
    ) -> tonic::Result<Response<proto::config::PaymentsConfigResponse>> {
        let config = self.services.payments.config();
        let response =
            proto::config::PaymentsConfigResponse { minimum_add_funds_payment: config.minimum_add_funds_payment };
        Ok(Response::new(response))
    }

    #[instrument(name = "api.payments.account_balance", skip_all, fields(user_id = request.trace_user_id()))]
    async fn account_balance(
        &self,
        request: tonic::Request<()>,
    ) -> tonic::Result<Response<proto::balance::AccountBalanceResponse>> {
        let user = request.user_id()?;
        let balance = self.services.payments.account_balance(&user).await.map_err(|e| {
            error!("Failed to lookup balance: {e}");
            Status::internal("failed to lookup balance")
        })?;
        let expires_at = balance.updated_at.add(self.balance_expiration);
        let response =
            AccountBalanceResponse { balance: balance.balance, last_updated: balance.updated_at, expires_at }
                .into_proto();
        Ok(Response::new(response))
    }

    #[instrument(name = "api.payments.add_funds", skip_all, fields(user_id = request.trace_user_id()))]
    async fn add_funds(&self, request: tonic::Request<proto::balance::AddFundsRequest>) -> tonic::Result<Response<()>> {
        let request: AddFundsRequest = request.into_inner().try_into_rust()?;
        self.services.payments.add_funds(request).await?;
        Ok(Response::new(()))
    }
}

impl From<QuoteError> for Status {
    fn from(e: QuoteError) -> Status {
        use QuoteError::*;
        match e {
            ProcessingProgram(_) | UnsatisfiablePreprocessingRequirements(..) | PayloadSize { .. } => {
                Status::invalid_argument(e.to_string())
            }
            AuxiliaryMaterialMissing => Status::unavailable(e.to_string()),
            Internal(e) => {
                error!("Failed to generate quote: {e}");
                Status::internal("internal error")
            }
        }
    }
}

impl From<PaymentVerificationError> for Status {
    fn from(e: PaymentVerificationError) -> Self {
        use PaymentVerificationError::*;
        match e {
            QuoteExpired | ReusedNonce => Self::failed_precondition(e.to_string()),
            NotEnoughFunds => {
                let mut details = ErrorDetails::new();
                details.set_precondition_failure(vec![PreconditionViolation::new(
                    "PAYMENT",
                    "BALANCE",
                    "balance is not enough to cover for operation cost",
                )]);
                Self::with_error_details(Code::FailedPrecondition, e.to_string(), details)
            }
            InsufficientPayment | NonceMismatch | InvalidSignature => Self::invalid_argument(e.to_string()),
            TransactionNotCommitted | TransactionNotFound => Self::unavailable(e.to_string()),
            NotEnoughElements(element) => {
                let mut details = ErrorDetails::new();
                details.set_quota_failure(vec![crate::grpc::quotas::PREPROCESSING.clone()]);
                Self::with_error_details(Code::ResourceExhausted, format!("not enough {element} elements"), details)
            }
            Internal(e) => {
                error!("Failed to verify payment: {e}");
                Self::internal("internal error")
            }
        }
    }
}

impl From<AddFundsError> for Status {
    fn from(e: AddFundsError) -> Self {
        use AddFundsError::*;
        match e {
            InvalidPayload | HashMismatch | ReusedTransaction | PaymentTooSmall => {
                Status::invalid_argument(e.to_string())
            }
            TransactionNotFound | TransactionNotCommitted => Status::unavailable(e.to_string()),
            Internal(e) => {
                error!("Failed to add funds: {e}");
                Status::internal("failed to add funds")
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        controllers::tests::PriceQuoteExt, services::payments::MockPaymentService, stateful::sm::StateMachineHandle,
    };
    use chrono::{DateTime, Utc};
    use mockall::predicate::{always, eq};
    use node_api::{
        payments::rust::{
            InvokeCompute, OperationMetadata, PreprocessingRequirement, PriceQuote, QuoteFees, Receipt, SignedQuote,
            SignedReceipt,
        },
        preprocessing::rust::PreprocessingElement,
        ConvertProto, Message,
    };
    use proto::payments_server::Payments;
    use rstest::rstest;
    use tokio::{spawn, sync::mpsc::channel};
    use user_keypair::SigningKey;

    struct PriceQuoteBuilder {
        nonce: Vec<u8>,
        fees: QuoteFees,
        request: Option<PriceQuoteRequest>,
        expires_at: DateTime<Utc>,
        preprocessing_requirements: Vec<PreprocessingRequirement>,
    }

    impl PriceQuoteBuilder {
        fn new(request: PriceQuoteRequest) -> Self {
            Self {
                nonce: vec![1, 2, 3],
                fees: Default::default(),
                request: Some(request),
                expires_at: Utc::now(),
                preprocessing_requirements: vec![],
            }
        }

        fn preprocessing_requirement(mut self, element: PreprocessingElement, count: u64) -> Self {
            self.preprocessing_requirements.push(PreprocessingRequirement { element, count });
            self
        }

        fn build(self) -> PriceQuote {
            let Self { nonce, fees, request, expires_at, preprocessing_requirements } = self;
            PriceQuote {
                nonce,
                fees,
                request: request.expect("no request set"),
                expires_at,
                preprocessing_requirements,
                auxiliary_material_requirements: Default::default(),
            }
        }
    }

    struct ServiceBuilder {
        payments: MockPaymentService,
        compute_handles: ComputeHandles,
        balance_expiration: Days,
    }

    impl Default for ServiceBuilder {
        fn default() -> Self {
            Self { payments: Default::default(), compute_handles: Default::default(), balance_expiration: Days::new(1) }
        }
    }

    impl ServiceBuilder {
        fn build(self) -> PaymentsApi {
            PaymentsApi::new(
                self.compute_handles,
                0,
                PaymentsApiServices { payments: Arc::new(self.payments) },
                self.balance_expiration,
            )
        }

        fn expect_payment_verification(&mut self, request: &PaymentReceiptRequest, metadata: OperationMetadata) {
            let quote = PriceQuote::try_decode(&request.signed_quote.quote).expect("invalid quote");
            self.payments.expect_generate_payment_receipt().with(always(), eq(metadata.clone())).return_once(
                |identifier, metadata| {
                    let receipt = Receipt { identifier, metadata, expires_at: Utc::now() }.into_proto().encode_to_vec();
                    Ok(SignedReceipt { receipt, signature: vec![42] })
                },
            );
            self.payments
                .expect_verify_payment()
                .with(eq(quote.clone()), eq(request.tx_hash.clone().unwrap()))
                .return_once(move |_, _| Ok(metadata));
            self.payments
                .expect_verify_decode_quote()
                .with(eq(request.signed_quote.clone()))
                .return_once(|_| Ok(quote));
        }
    }

    #[rstest]
    #[tokio::test]
    async fn price_quote() {
        let request = PriceQuoteRequest::PoolStatus;
        let mut builder = ServiceBuilder::default();
        let expected_quote = SignedQuote { quote: vec![1, 2, 3], signature: vec![4, 5, 6] };
        {
            let expected_quote = expected_quote.clone();
            builder.payments.expect_generate_quote().with(eq(request.clone())).return_once(move |_| Ok(expected_quote));
        }

        let api = builder.build();
        let quote =
            api.price_quote(Request::new(request.into_proto())).await.expect("failed to get quote").into_inner();
        assert_eq!(quote, expected_quote);
    }

    #[tokio::test]
    async fn price_quote_fail() {
        let mut builder = ServiceBuilder::default();
        builder.payments.expect_generate_quote().return_once(move |_| Err(QuoteError::Internal("error".into())));

        let api = builder.build();
        api.price_quote(Request::new(PriceQuoteRequest::PoolStatus.into_proto()))
            .await
            .expect_err("getting quote succeeded");
    }

    #[tokio::test]
    async fn payment_receipt() {
        let keypair = SigningKey::generate_secp256k1();
        let tx_hash = "hash".to_string();
        let quote = PriceQuoteBuilder::new(PriceQuoteRequest::PoolStatus).build();
        let request = quote.receipt_request(&tx_hash, &keypair);
        let mut builder = ServiceBuilder::default();

        builder.expect_payment_verification(&request, OperationMetadata::PoolStatus);

        let api = builder.build();
        let response = api
            .payment_receipt(Request::new(request.into_proto()))
            .await
            .expect("processing request failed")
            .into_inner();
        Receipt::try_decode(&response.receipt).expect("invalid receipt");
    }

    #[tokio::test]
    async fn too_many_computes() {
        let keypair = SigningKey::generate_secp256k1();
        let tx_hash = "hash".to_string();
        let quote_inner = InvokeCompute { program_id: "foo".into(), values_payload_size: 0 };
        let quote = PriceQuoteBuilder::new(PriceQuoteRequest::InvokeCompute(quote_inner.clone()))
            .preprocessing_requirement(PreprocessingElement::Compare, 100)
            .build();
        let request = quote.receipt_request(&tx_hash, &keypair);
        let mut builder = ServiceBuilder::default();

        let quote = PriceQuote::try_decode(&request.signed_quote.quote).expect("invalid quote");
        builder
            .payments
            .expect_verify_payment()
            .with(eq(quote.clone()), eq(request.tx_hash.clone().unwrap()))
            .return_once(move |_, _| Ok(OperationMetadata::PoolStatus));
        builder.payments.expect_verify_decode_quote().with(eq(request.signed_quote.clone())).return_once(|_| Ok(quote));

        // pretend like there's one running
        builder
            .compute_handles
            .lock()
            .await
            .insert(Uuid::new_v4(), StateMachineHandle { init_sender: channel(1).0, join_handle: spawn(async {}) });

        let api = builder.build();
        let response =
            api.payment_receipt(Request::new(request.into_proto())).await.expect_err("processing request succeed");
        assert_eq!(response.code(), Code::Unavailable);
    }

    #[tokio::test]
    async fn add_funds() {
        let mut builder = ServiceBuilder::default();
        let request = AddFundsRequest { payload: vec![1, 2, 3], tx_hash: "hash".into() };
        builder.payments.expect_add_funds().with(eq(request.clone())).return_once(|_| Ok(()));
        builder.build().add_funds(Request::new(request)).await.expect("adding funds failed");
    }
}
