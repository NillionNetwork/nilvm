//! Add funds operation.

use super::{BuildError, InitialState, InitialStateInvokeError, InvokeError, PaymentError};
use crate::{payments::TxHash, vm::VmClient};
use nillion_chain_client::transactions::TokenAmount;
use node_api::{
    auth::rust::UserId,
    payments::rust::{AddFundsPayload, AddFundsRequest},
    ConvertProto, Message,
};
use rand::random;
use sha2::{Digest, Sha256};
use tracing::info;

/// An operation that add funds to a recipient's account.
pub struct AddFundsOperation<'a, S> {
    client: &'a VmClient,
    recipient: UserId,
    amount: TokenAmount,
    payload: Vec<u8>,
    state: S,
}

impl<'a> AddFundsOperation<'a, InitialState> {
    /// Pay for this operation.
    pub async fn pay(self) -> Result<AddFundsOperation<'a, SimplePaidState>, PaymentError> {
        let Self { client, recipient, amount, payload, .. } = self;

        // Ensure the user is paying the minimum configured amount.
        let config = client
            .payments
            .payments_config()
            .await
            .map_err(|e| PaymentError(format!("failed to get payments config: {e}")))?;
        if amount.to_unil() < config.minimum_add_funds_payment {
            return Err(PaymentError(format!("minimum payment is {} unil", config.minimum_add_funds_payment)));
        }

        let payload_hash = Sha256::digest(&payload).to_vec();
        let tx_hash = self
            .client
            .nilchain_payer
            .submit_payment(amount.to_unil(), payload_hash)
            .await
            .map_err(|e| PaymentError(e.to_string()))?;
        info!("Payment for add funds operation done in {tx_hash}");
        Ok(AddFundsOperation { client, recipient, amount, payload, state: SimplePaidState { tx_hash } })
    }

    /// Make the payment and invoke this operation all at once.
    pub async fn invoke(self) -> Result<TxHash, InitialStateInvokeError> {
        Ok(self.pay().await?.invoke().await?)
    }
}

impl<'a> AddFundsOperation<'a, SimplePaidState> {
    /// Invoke the operation and effectively add funds to the recipient account.
    pub async fn invoke(self) -> Result<TxHash, InvokeError> {
        let Self { client, recipient, amount, payload, state } = self;
        let tx_hash = state.tx_hash;
        let request = AddFundsRequest { payload, tx_hash: tx_hash.clone().into() };
        client.payments.add_funds(request).await?;
        info!("{amount} tokens added to recipient {recipient}");
        Ok(tx_hash)
    }
}

/// A simple paid state that only contains a transaction hash.
pub struct SimplePaidState {
    /// The transaction hash for the payment made.
    pub tx_hash: TxHash,
}

/// A builder for the add funds operation.
#[must_use]
pub struct AddFundsOperationBuilder<'a> {
    client: &'a VmClient,
    recipient: UserId,
    amount: TokenAmount,
}

impl<'a> AddFundsOperationBuilder<'a> {
    pub(crate) fn new(client: &'a VmClient) -> Self {
        Self { client, recipient: client.user_id(), amount: TokenAmount::Unil(0) }
    }

    /// Set the recipient of these funds.
    pub fn recipient(mut self, recipient: UserId) -> Self {
        self.recipient = recipient;
        self
    }

    /// Set the amount of funds to be added.
    pub fn amount(mut self, amount: TokenAmount) -> Self {
        self.amount = amount;
        self
    }

    /// Build the operation.
    pub fn build(self) -> Result<AddFundsOperation<'a, InitialState>, BuildError> {
        let Self { client, recipient, amount } = self;
        if amount.to_unil() == 0 {
            return Err(BuildError("amount must be > 0".into()));
        }
        let leader_public_key = Some(client.cluster.leader.public_keys.authentication.clone());
        let payload = AddFundsPayload { recipient, nonce: random(), leader_public_key }.into_proto().encode_to_vec();
        Ok(AddFundsOperation { client, recipient, amount, payload, state: InitialState })
    }
}
