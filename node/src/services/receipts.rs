//! Receipts service.

use super::{
    nonce::{NonceService, RecordNonceError},
    time::TimeService,
};
use crate::{services::payments::Nonce, storage::repositories::nonces::ExpireableNonce};
use async_trait::async_trait;
use node_api::{
    payments::rust::{Receipt, SignedReceipt},
    ConvertProto,
};
use std::sync::Arc;
use tracing::info;
use user_keypair::{PublicKey, Signature};

/// Receipts service.
#[cfg_attr(test, mockall::automock)]
#[async_trait]
pub(crate) trait ReceiptsService: Send + Sync + 'static {
    /// Verify and decode a payment receipt.
    async fn verify_payment_receipt(&self, signed_receipt: SignedReceipt) -> Result<Receipt, ReceiptVerificationError>;
}

#[derive(Debug, thiserror::Error)]
pub(crate) enum ReceiptVerificationError {
    #[error("invalid signature")]
    InvalidSignature,

    #[error("quote expired")]
    QuoteExpired,

    #[error("nonce already used")]
    ReusedNonce,

    #[error("{0}")]
    Internal(String),
}

impl From<RecordNonceError> for ReceiptVerificationError {
    fn from(e: RecordNonceError) -> Self {
        use RecordNonceError::*;
        match e {
            Internal(e) => Self::Internal(e),
            ReusedNonce => Self::ReusedNonce,
        }
    }
}

pub(crate) struct DefaultReceiptsService {
    leader_public_key: PublicKey,
    time_service: Arc<dyn TimeService>,
    nonce_service: Arc<dyn NonceService>,
}

impl DefaultReceiptsService {
    pub(crate) fn new(
        leader_public_key: PublicKey,
        time_service: Arc<dyn TimeService>,
        nonce_service: Arc<dyn NonceService>,
    ) -> Self {
        Self { leader_public_key, time_service, nonce_service }
    }
}

#[async_trait]
impl ReceiptsService for DefaultReceiptsService {
    async fn verify_payment_receipt(&self, signed_receipt: SignedReceipt) -> Result<Receipt, ReceiptVerificationError> {
        let signature = Signature::from(signed_receipt.signature);
        self.leader_public_key
            .verify(&signature, &signed_receipt.receipt)
            .map_err(|_| ReceiptVerificationError::InvalidSignature)?;
        let receipt = Receipt::try_decode(&signed_receipt.receipt)
            .map_err(|e| ReceiptVerificationError::Internal(e.to_string()))?;

        let nonce = Nonce(receipt.identifier.clone());
        info!("Marking nonce {nonce} as used");
        self.nonce_service.record_nonce(&ExpireableNonce::new_receipt(nonce, receipt.expires_at)).await?;
        if receipt.expires_at < self.time_service.current_time() {
            Err(ReceiptVerificationError::QuoteExpired)
        } else {
            Ok(receipt)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::{nonce::MockNonceService, time::MockTimeService};
    use chrono::Utc;
    use mockall::predicate::eq;
    use node_api::{payments::rust::OperationMetadata, Message};
    use std::time::Duration;
    use user_keypair::SigningKey;

    struct ServiceBuilder {
        leader_public_key: PublicKey,
        time_service: MockTimeService,
        nonce_service: MockNonceService,
    }

    impl ServiceBuilder {
        fn build(self) -> DefaultReceiptsService {
            DefaultReceiptsService::new(
                self.leader_public_key,
                Arc::new(self.time_service),
                Arc::new(self.nonce_service),
            )
        }
    }

    impl Default for ServiceBuilder {
        fn default() -> Self {
            let mut time_service = MockTimeService::default();
            time_service.expect_current_time().returning(|| Utc::now());
            Self {
                leader_public_key: SigningKey::generate_secp256k1().public_key(),
                time_service,
                nonce_service: Default::default(),
            }
        }
    }

    #[tokio::test]
    async fn verify_payment_receipt() {
        let keypair = SigningKey::generate_secp256k1();
        let nonce = vec![1, 2, 3];
        let expires_at = Utc::now() + Duration::from_secs(60);
        let receipt = Receipt { identifier: nonce.clone(), metadata: OperationMetadata::PoolStatus, expires_at };
        let serialized_receipt = receipt.clone().into_proto().encode_to_vec();
        let signature = keypair.sign(&serialized_receipt).into();
        let signed_receipt = SignedReceipt { receipt: serialized_receipt, signature };

        let mut builder = ServiceBuilder { leader_public_key: keypair.public_key(), ..Default::default() };
        builder
            .nonce_service
            .expect_record_nonce()
            .with(eq(ExpireableNonce::new_receipt(Nonce(nonce.clone()), expires_at)))
            .return_once(|_| Ok(()));
        let service = builder.build();
        let decoded_receipt = service.verify_payment_receipt(signed_receipt).await.expect("validation failed");
        assert_eq!(decoded_receipt, receipt);
    }

    #[tokio::test]
    async fn verify_payment_receipt_invalid_signature() {
        let receipt = Receipt {
            identifier: vec![1, 2, 3],
            metadata: OperationMetadata::PoolStatus,
            expires_at: Utc::now() + Duration::from_secs(60),
        };
        let serialized_receipt = receipt.clone().into_proto().encode_to_vec();
        let signed_receipt = SignedReceipt { receipt: serialized_receipt, signature: vec![1, 2, 3] };

        let service = ServiceBuilder::default().build();
        service.verify_payment_receipt(signed_receipt).await.expect_err("validation succeeded");
    }

    #[tokio::test]
    async fn verify_payment_receipt_invalid_payload() {
        let keypair = SigningKey::generate_secp256k1();
        let receipt = vec![1, 3, 3, 7];
        let signature = keypair.sign(&receipt).into();
        let signed_receipt = SignedReceipt { receipt, signature };

        let service = ServiceBuilder { leader_public_key: keypair.public_key(), ..Default::default() }.build();
        service.verify_payment_receipt(signed_receipt).await.expect_err("validation succeeded");
    }
}
