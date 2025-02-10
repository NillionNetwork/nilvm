//! Payments gRPC client.

use grpc_channel::{AuthenticatedGrpcChannel, TransportChannel};
use node_api::{payments::proto, ConvertProto, TryIntoRust};

pub use node_api::payments::rust::*;

/// A client that interacts with the payments service.
#[derive(Clone)]
pub struct PaymentsClient(
    proto::payments_client::PaymentsClient<<AuthenticatedGrpcChannel as TransportChannel>::Channel>,
);

impl PaymentsClient {
    /// Create a new client.
    pub fn new(channel: AuthenticatedGrpcChannel) -> Self {
        let client = proto::payments_client::PaymentsClient::new(channel.into_channel());
        Self(client)
    }

    /// Get a price quote for an operation in the network.
    pub async fn price_quote(&self, request: PriceQuoteRequest) -> tonic::Result<SignedQuote> {
        let response = self.0.clone().price_quote(request.into_proto()).await?;
        Ok(response.into_inner().try_into_rust()?)
    }

    /// Generate a payment receipt for a payment.
    pub async fn payment_receipt(&self, request: PaymentReceiptRequest) -> tonic::Result<SignedReceipt> {
        let response = self.0.clone().payment_receipt(request.into_proto()).await?;
        Ok(response.into_inner().try_into_rust()?)
    }

    /// Get the payments configuration.
    pub async fn payments_config(&self) -> tonic::Result<PaymentsConfigResponse> {
        let response = self.0.clone().payments_config(()).await?;
        Ok(response.into_inner())
    }

    /// Get the balance for our account.
    pub async fn account_balance(&self) -> tonic::Result<AccountBalanceResponse> {
        let response = self.0.clone().account_balance(()).await?;
        Ok(response.into_inner().try_into_rust()?)
    }

    /// Add funds to a user's balance.
    pub async fn add_funds(&self, request: AddFundsRequest) -> tonic::Result<()> {
        self.0.clone().add_funds(request.into_proto()).await?;
        Ok(())
    }
}
