//! The entry points for the node's controllers.

use crate::{
    services::{nonce::RecordNonceError, receipts::ReceiptVerificationError},
    storage::{models::program::ParseProgramIdError, repositories::blob::BlobRepositoryError},
};
use encoding::codec::MessageCodec;
use grpc_channel::auth::AuthenticateRequest;
use math_lib::modular::EncodedModulo;
use nada_value::protobuf::{nada_values_from_protobuf, nada_values_to_protobuf};
use node_api::values::rust::NamedValue;
use tonic::{Request, Status};
use tracing::error;

pub(crate) mod compute;
pub(crate) mod leader_queries;
pub(crate) mod membership;
pub(crate) mod payments;
pub(crate) mod permissions;
pub(crate) mod preprocessing;
pub(crate) mod programs;
pub(crate) mod values;

impl From<RecordNonceError> for Status {
    fn from(e: RecordNonceError) -> Self {
        use RecordNonceError::*;
        match e {
            ReusedNonce => Status::failed_precondition(e.to_string()),
            Internal(e) => {
                error!("Failed to record nonce: {e}");
                Status::internal("internal error")
            }
        }
    }
}

#[derive(Debug, thiserror::Error)]
#[error("receipt is not for a '{0}' operation")]
pub(crate) struct InvalidReceiptType(&'static str);

impl From<InvalidReceiptType> for Status {
    fn from(e: InvalidReceiptType) -> Self {
        Self::invalid_argument(e.to_string())
    }
}

impl From<BlobRepositoryError> for Status {
    fn from(e: BlobRepositoryError) -> Self {
        use BlobRepositoryError::*;
        match e {
            NotFound => Status::not_found(e.to_string()),
            AlreadyExists => Status::failed_precondition(e.to_string()),
            Encode(_) | Decode(_) | Io(_) | Internal(_) => {
                error!("Blob repository operation failed: {e}");
                Status::internal("internal error")
            }
        }
    }
}

impl From<ReceiptVerificationError> for Status {
    fn from(e: ReceiptVerificationError) -> Self {
        use ReceiptVerificationError::*;
        match e {
            InvalidSignature => Status::invalid_argument(e.to_string()),
            QuoteExpired | ReusedNonce => Status::failed_precondition(e.to_string()),
            Internal(e) => {
                error!("Failed to verify receipt: {e}");
                Status::internal("internal error")
            }
        }
    }
}

impl From<ParseProgramIdError> for Status {
    fn from(e: ParseProgramIdError) -> Self {
        Self::invalid_argument(format!("invalid program id: {e}"))
    }
}

pub(crate) trait TraceRequest {
    fn trace_user_id(&self) -> String;
}

impl<T> TraceRequest for Request<T> {
    fn trace_user_id(&self) -> String {
        match self.user_id() {
            Ok(id) => id.to_string(),
            Err(_) => "<none>".to_string(),
        }
    }
}

pub(crate) fn extract_values(
    values: Vec<NamedValue>,
    bincode_values: &[u8],
    modulo: &EncodedModulo,
) -> tonic::Result<Vec<NamedValue>> {
    let values = match (values.is_empty(), bincode_values.is_empty()) {
        (true, false) => {
            let values = MessageCodec
                .decode(bincode_values)
                .map_err(|e| Status::invalid_argument(format!("invalid values bincode: {e}")))?;
            nada_values_to_protobuf(values).map_err(|e| Status::invalid_argument(format!("invalid values: {e}")))?
        }
        (false, true) => {
            // Convert them to ensure they don't validate any invariants
            nada_values_from_protobuf(values.clone(), modulo)
                .map_err(|e| Status::invalid_argument(format!("invalid values: {e}")))?;
            values
        }
        (false, false) => {
            return Err(Status::invalid_argument("only one of 'values' and 'bincode_values' must be set"));
        }
        (true, true) => Vec::new(),
    };
    Ok(values)
}

#[cfg(test)]
pub(crate) mod tests {
    use chrono::{DateTime, Utc};
    use grpc_channel::auth::AuthenticatedExtension;
    use node_api::{
        auth::rust::UserId,
        payments::rust::{OperationMetadata, PaymentReceiptRequest, PriceQuote, Receipt, SignedQuote, SignedReceipt},
        ConvertProto, Message,
    };
    use user_keypair::SigningKey;

    pub(crate) trait MakeAuthenticated: Sized {
        fn authenticated(self, user_id: UserId) -> Self;
    }

    impl<T> MakeAuthenticated for tonic::Request<T> {
        fn authenticated(mut self, user_id: UserId) -> Self {
            self.extensions_mut().insert(AuthenticatedExtension(user_id));
            self
        }
    }

    pub(crate) trait PriceQuoteExt {
        fn receipt_request(&self, tx_hash: &str, key: &SigningKey) -> PaymentReceiptRequest;
    }

    impl PriceQuoteExt for PriceQuote {
        fn receipt_request(&self, tx_hash: &str, key: &SigningKey) -> PaymentReceiptRequest {
            let serialized_quote = self.clone().into_proto().encode_to_vec();
            let signature = key.sign(&serialized_quote).into();
            PaymentReceiptRequest {
                signed_quote: SignedQuote { quote: serialized_quote, signature },
                tx_hash: Some(tx_hash.to_string()),
            }
        }
    }

    pub(crate) struct ReceiptBuilder {
        identifier: Vec<u8>,
        metadata: OperationMetadata,
        expires_at: DateTime<Utc>,
    }

    impl ReceiptBuilder {
        pub(crate) fn new<T: Into<OperationMetadata>>(metadata: T) -> Self {
            Self { identifier: vec![1, 2, 3], metadata: metadata.into(), expires_at: Utc::now() }
        }

        pub(crate) fn build(self) -> Receipt {
            let Self { identifier, metadata, expires_at } = self;
            Receipt { identifier, metadata, expires_at }
        }
    }

    pub(crate) fn empty_signed_receipt() -> SignedReceipt {
        SignedReceipt { signature: Vec::new(), receipt: Vec::new() }
    }
}
