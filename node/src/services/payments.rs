use super::{
    auxiliary_material::AuxiliaryMaterialMetadataService,
    offsets::{ElementOffsetsService, RequestOffsetsError},
    programs::ProgramService,
    time::TimeService,
};
use crate::{
    services::token_dollar_conversion::{TokenDollarConversionError, TokenDollarConversionService},
    storage::{
        models::program::{ParseProgramIdError, ProgramId},
        repositories::{
            balances::{AccountBalance, AccountBalanceRepository},
            nonces::{ExpireableNonce, UsedNoncesRepository},
            transfers::{Transfer, TransfersRepository},
        },
        sqlite::{DatabaseError, TransactionContext},
    },
    PreprocessingConfigExt,
};
use axum::async_trait;
use metrics::prelude::*;
use mpc_vm::requirements::{MPCProgramRequirements, RuntimeRequirementType};
use nillion_chain_client::{
    transactions::TokenAmount,
    tx::{PaymentTransactionRetriever, RetrieveError},
};
use node_api::{
    auth::rust::UserId,
    compute::{TECDSA_DKG_PROGRAM_ID, TEDDSA_DKG_PROGRAM_ID},
    payments::rust::{
        AddFundsPayload, AddFundsRequest, AuxiliaryMaterialRequirement, InvokeCompute, InvokeComputeMetadata,
        OperationMetadata, PreprocessingRequirement, PriceQuote, PriceQuoteRequest, QuoteFees, Receipt,
        SelectedAuxiliaryMaterial, SelectedPreprocessingOffsets, SignedQuote, SignedReceipt,
    },
    preprocessing::rust::{AuxiliaryMaterial, PreprocessingElement},
    ConvertProto, Message,
};
use node_config::{PreprocessingConfig, PricingConfig};
use once_cell::sync::Lazy;
use program_auditor::ProgramAuditorRequest;
use rand::random;
use rust_decimal::{prelude::ToPrimitive, Decimal};
use sha2::{Digest, Sha256};
use std::{collections::HashSet, fmt, ops::Add, sync::Arc, time::Duration};
use tracing::{error, info, warn};
use user_keypair::{InvalidSignature, Signature, SigningKey};

const NONCE_LENGTH: usize = 32;
const MINIMUM_PAYMENT_THRESHOLD: f64 = 0.9;

static METRICS: Lazy<Metrics> = Lazy::new(Metrics::default);

/// A nonce.
#[derive(Debug, Clone, PartialEq)]
pub struct Nonce(pub Vec<u8>);

// This implementation is temporary; once we use the blockchain we should drop it.
impl Default for Nonce {
    fn default() -> Self {
        Nonce(vec![0; 32])
    }
}

impl fmt::Display for Nonce {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", hex::encode(&self.0))
    }
}

#[cfg_attr(test, mockall::automock)]
#[async_trait]
pub(crate) trait PaymentService: Send + Sync + 'static {
    /// Generates a quote for an operation.
    async fn generate_quote(&self, request: PriceQuoteRequest) -> Result<SignedQuote, QuoteError>;

    /// Decode and verify a signed quote.
    fn verify_decode_quote(&self, signed_quote: SignedQuote) -> Result<PriceQuote, PaymentVerificationError>;

    /// Validate that the quote is valid, the payment was made, and the quote is not yet expired.
    async fn verify_payment(
        &self,
        quote: PriceQuote,
        tx_hash: String,
    ) -> Result<OperationMetadata, PaymentVerificationError>;

    /// Deduct the payment for this signed quote from the user account's balance.
    async fn deduct_payment_from_balance(
        &self,
        quote: PriceQuote,
        user_id: &UserId,
    ) -> Result<OperationMetadata, PaymentVerificationError>;

    /// Sign a receipt with the given parameters.
    fn generate_payment_receipt(
        &self,
        identifier: Vec<u8>,
        metadata: OperationMetadata,
    ) -> Result<SignedReceipt, InvalidSignature>;

    /// Get a user account's balance.
    async fn account_balance(&self, user: &UserId) -> Result<AccountBalance, BalanceLookupError>;

    /// Add funds to a user's account.
    async fn add_funds(&self, request: AddFundsRequest) -> Result<(), AddFundsError>;

    /// The minimum add funds payment accepted.
    async fn minimum_add_funds_payment(&self) -> Result<TokenAmount, MinimumAddFundsPaymentError>;

    /// The credits conversion rate for 1 nil.
    async fn nil_credits_conversion_rate(&self) -> Result<u64, ConversionRateError>;
}

/// An error when fetching the minimum add funds payment.
#[derive(thiserror::Error, Debug, Clone, PartialEq)]
#[error("internal: {0}")]
pub(crate) struct MinimumAddFundsPaymentError(String);

/// An error when the nil credits conversion rate.
#[derive(thiserror::Error, Debug, Clone, PartialEq)]
#[error("internal: {0}")]
pub(crate) struct ConversionRateError(String);

/// A quoting error.
#[derive(thiserror::Error, Debug, Clone, PartialEq)]
pub(crate) enum QuoteError {
    /// Looking up program failed.
    #[error("processing target program failed: {0}")]
    ProcessingProgram(String),

    /// The operation requires more preprocessing that the cluster is configured to generate.
    #[error("cluster is only configured to generate at most {1} {0} preprocessing elements")]
    UnsatisfiablePreprocessingRequirements(PreprocessingElement, u64),

    /// The payload size is too large.
    #[error("payload size ({request}) exceeds maximum ({maximum})")]
    PayloadSize {
        /// The request's payload size.
        request: u64,

        /// The maximum payload size.
        maximum: u64,
    },

    /// Auxiliary material is not generated yet.
    #[error("auxiliary material is missing")]
    AuxiliaryMaterialMissing,

    /// Token dollar conversion error.
    #[error("token dollar conmversion: {0}")]
    TokenDollarConversion(#[from] TokenDollarConversionError),

    /// An internal error.
    #[error("internal: {0}")]
    Internal(String),
}

#[derive(Debug, thiserror::Error)]
pub(crate) enum PaymentVerificationError {
    #[error("invalid signature")]
    InvalidSignature,

    #[error("quote expired")]
    QuoteExpired,

    #[error("nonce in payment doens't match nonce in quote")]
    NonceMismatch,

    #[error("nonce already used")]
    ReusedNonce,

    #[error("insufficient payment")]
    InsufficientPayment,

    #[error("transaction not found")]
    TransactionNotFound,

    #[error("transaction not committed")]
    TransactionNotCommitted,

    #[error("not enough funds")]
    NotEnoughFunds,

    #[error("not enough elements: {0}")]
    NotEnoughElements(PreprocessingElement),

    #[error("{0}")]
    Internal(String),
}

impl From<RequestOffsetsError> for PaymentVerificationError {
    fn from(e: RequestOffsetsError) -> Self {
        match e {
            RequestOffsetsError::Repository(e) => Self::from(e),
            RequestOffsetsError::NotEnoughElements(element) => Self::NotEnoughElements(element),
            RequestOffsetsError::Transaction(e) => Self::Internal(e.to_string()),
            RequestOffsetsError::Internal(e) => Self::Internal(e),
        }
    }
}

impl From<TransactionFetchError> for PaymentVerificationError {
    fn from(error: TransactionFetchError) -> Self {
        use TransactionFetchError::*;
        match error {
            Internal(e) => Self::Internal(e),
            NotCommitted => Self::TransactionNotCommitted,
            NotFound => Self::TransactionNotFound,
        }
    }
}

impl From<DatabaseError> for PaymentVerificationError {
    fn from(e: DatabaseError) -> Self {
        match e {
            DatabaseError::UniqueConstraint => Self::ReusedNonce,
            DatabaseError::Constraint | DatabaseError::Execution(sqlx::Error::RowNotFound) => Self::NotEnoughFunds,
            _ => Self::Internal(e.to_string()),
        }
    }
}

/// A add funds error.
#[derive(thiserror::Error, Debug)]
pub(crate) enum AddFundsError {
    #[error("invalid payload")]
    InvalidPayload,

    #[error("payment is too small")]
    PaymentTooSmall,

    #[error("hash in payment does not match hash of payload")]
    HashMismatch,

    #[error("payment transaction has already been processed")]
    ReusedTransaction,

    #[error("transaction not found")]
    TransactionNotFound,

    #[error("transaction not committed")]
    TransactionNotCommitted,

    /// Token dollar conversion error.
    #[error("token dollar conversion: {0}")]
    TokenDollarConversion(#[from] TokenDollarConversionError),

    #[error("{0}")]
    Internal(String),
}

impl From<DatabaseError> for AddFundsError {
    fn from(e: DatabaseError) -> Self {
        match e {
            DatabaseError::UniqueConstraint => Self::ReusedTransaction,
            DatabaseError::ConnectionAcquire(_)
            | DatabaseError::Constraint
            | DatabaseError::NotFound
            | DatabaseError::Execution(_)
            | DatabaseError::IntegerOverflow => Self::Internal(e.to_string()),
        }
    }
}

impl From<TransactionFetchError> for AddFundsError {
    fn from(error: TransactionFetchError) -> Self {
        use TransactionFetchError::*;
        match error {
            Internal(e) => Self::Internal(e),
            NotCommitted => Self::TransactionNotCommitted,
            NotFound => Self::TransactionNotFound,
        }
    }
}

#[derive(Debug, thiserror::Error)]
#[error("{0}")]
pub(crate) struct BalanceLookupError(String);

pub(crate) struct PaymentServiceDependencies {
    pub(crate) time_service: Arc<dyn TimeService>,
    pub(crate) programs_service: Arc<dyn ProgramService>,
    pub(crate) balance_repo: Arc<dyn AccountBalanceRepository>,
    pub(crate) transfers_repo: Arc<dyn TransfersRepository>,
    pub(crate) used_nonces_repo: Arc<dyn UsedNoncesRepository>,
    pub(crate) tx_retriever: Arc<dyn PaymentTransactionRetriever>,
    pub(crate) offsets_service: Arc<dyn ElementOffsetsService>,
    pub(crate) auxiliary_material_metadata_service: Arc<dyn AuxiliaryMaterialMetadataService>,
    pub(crate) token_dollar_conversion_service: Arc<dyn TokenDollarConversionService>,
}

pub(crate) struct PaymentsServiceConfig {
    pub(crate) max_payload_size: u64,
    pub(crate) pricing: PricingConfig,
    pub(crate) preprocessing: PreprocessingConfig,
    pub(crate) quote_ttl: Duration,
    pub(crate) receipt_ttl: Duration,
    pub(crate) minimum_add_funds_credits: Decimal,
}

pub(crate) struct DefaultPaymentService {
    signing_key: SigningKey,
    dependencies: PaymentServiceDependencies,
    config: PaymentsServiceConfig,
}

impl DefaultPaymentService {
    pub(crate) fn new(
        signing_key: SigningKey,
        dependencies: PaymentServiceDependencies,
        config: PaymentsServiceConfig,
    ) -> Self {
        Self { signing_key, dependencies, config }
    }

    fn calculate_cost(
        &self,
        operation: &PriceQuoteRequest,
        token_dollar_price_cents: Decimal,
    ) -> Result<QuoteFees, QuoteError> {
        let cost = match operation {
            PriceQuoteRequest::PoolStatus => self.config.pricing.pool_status_price,
            PriceQuoteRequest::RetrievePermissions(_) => self.config.pricing.retrieve_permissions_price,
            PriceQuoteRequest::OverwritePermissions(_) => self.config.pricing.overwrite_permissions_price,
            PriceQuoteRequest::UpdatePermissions(_) => self.config.pricing.update_permissions_price,
            PriceQuoteRequest::RetrieveValues(_) => self.config.pricing.retrieve_values_price,
            PriceQuoteRequest::StoreProgram(_) => self.config.pricing.store_program_price,
            PriceQuoteRequest::StoreValues(_) => self.config.pricing.store_values_price,
            PriceQuoteRequest::InvokeCompute(_) => self.config.pricing.invoke_compute_price,
        };
        let cost_cents = Decimal::from(cost);
        let tokens_nil = cost_cents
            .checked_div(token_dollar_price_cents)
            .ok_or(QuoteError::Internal("division error".to_string()))?;
        let tokens_unil =
            tokens_nil.checked_mul(Decimal::from(1_000_000)).ok_or(QuoteError::Internal("overflow".to_string()))?;
        let tokens_unil = tokens_unil.to_u64().ok_or(QuoteError::Internal("conversion error".to_string()))?;
        let tokens = TokenAmount::Unil(tokens_unil).to_unil();
        Ok(QuoteFees { tokens, credits: cost })
    }

    async fn fetch_transaction(&self, hash: &str) -> Result<Transaction, TransactionFetchError> {
        let result = self.dependencies.tx_retriever.get(hash).await;
        match result {
            Ok(tx) => {
                let tx = Transaction { nonce: Nonce(tx.resource), paid_amount: tx.amount.to_unil() };
                info!("Found transaction {hash}, amount paid {}, nonce {}", tx.paid_amount, tx.nonce);
                Ok(tx)
            }
            Err(RetrieveError::NotCommitted) => Err(TransactionFetchError::NotCommitted),
            Err(RetrieveError::TransactionFetch(e)) => {
                info!("Failed to fetch transaction: {e}");
                Err(TransactionFetchError::NotFound)
            }
            Err(e) => {
                METRICS.inc_transaction_fetch_errors();
                Err(TransactionFetchError::Internal(e.to_string()))
            }
        }
    }

    fn validate_requirements(&self, requirements: &[PreprocessingRequirement]) -> Result<(), QuoteError> {
        for element_requirement in requirements {
            let PreprocessingRequirement { element, count } = element_requirement;
            let config = self.config.preprocessing.element_config(element);
            // Note that in reality we have slightly more than this but this is the "safe bet" that
            // we will eventually generate for sure.
            if *count > config.generation_threshold {
                return Err(QuoteError::UnsatisfiablePreprocessingRequirements(*element, config.generation_threshold));
            }
        }
        Ok(())
    }

    fn validate_payload_size(&self, request: &PriceQuoteRequest) -> Result<(), QuoteError> {
        let payload_size = match request {
            PriceQuoteRequest::StoreValues(metadata) => metadata.payload_size,
            PriceQuoteRequest::InvokeCompute(metadata) => metadata.values_payload_size,
            PriceQuoteRequest::StoreProgram(metadata) => metadata.metadata.program_size,
            _ => return Ok(()),
        };
        if payload_size > self.config.max_payload_size {
            Err(QuoteError::PayloadSize { request: payload_size, maximum: self.config.max_payload_size })
        } else {
            Ok(())
        }
    }

    fn validate_program(&self, request: &PriceQuoteRequest) -> Result<(), QuoteError> {
        // Audit quote for store program
        if let PriceQuoteRequest::StoreProgram(operation) = request {
            let request = ProgramAuditorRequest {
                memory_size: operation.metadata.memory_size,
                total_instructions: operation.metadata.instruction_count,
                instructions: operation.metadata.instructions.clone(),
                preprocessing_requirements: Self::convert_requirements(&operation.metadata.preprocessing_requirements),
            };

            self.dependencies
                .programs_service
                .audit(&request)
                .map_err(|e| QuoteError::ProcessingProgram(format!("program audit failed: {e}")))?;
        }
        Ok(())
    }

    async fn analyze_program(
        &self,
        request: &PriceQuoteRequest,
    ) -> Result<(Vec<PreprocessingRequirement>, Vec<AuxiliaryMaterialRequirement>), QuoteError> {
        use QuoteError::ProcessingProgram;
        if let PriceQuoteRequest::InvokeCompute(metadata) = &request {
            let program_id: ProgramId =
                metadata.program_id.parse().map_err(|e: ParseProgramIdError| ProcessingProgram(e.to_string()))?;
            let program_id_str = program_id.to_string();
            if program_id_str == TECDSA_DKG_PROGRAM_ID || program_id_str == TEDDSA_DKG_PROGRAM_ID {
                return Ok((vec![], vec![]));
            }
            let program = self
                .dependencies
                .programs_service
                .find(&program_id)
                .await
                .map_err(|e| QuoteError::ProcessingProgram(e.to_string()))?;
            let runtime_requirements = self
                .dependencies
                .programs_service
                .requirements(&program)
                .map_err(|e| QuoteError::ProcessingProgram(e.to_string()))?;
            let (preprocessing_requirements, auxiliary_materials_requirements) =
                self.transform_runtime_requirements(runtime_requirements).await?;
            Ok((preprocessing_requirements, auxiliary_materials_requirements))
        } else {
            Ok((vec![], vec![]))
        }
    }

    async fn transform_runtime_requirements(
        &self,
        runtime_requirements: MPCProgramRequirements,
    ) -> Result<(Vec<PreprocessingRequirement>, Vec<AuxiliaryMaterialRequirement>), QuoteError> {
        let mut preprocessing_requirements = Vec::new();
        let mut auxiliary_materials = HashSet::new();
        for (element, count) in runtime_requirements {
            use RuntimeRequirementType::*;
            let element = match element {
                Compare => PreprocessingElement::Compare,
                DivisionIntegerSecret => PreprocessingElement::DivisionSecretDivisor,
                Modulo => PreprocessingElement::Modulo,
                PublicOutputEquality => PreprocessingElement::EqualityPublicOutput,
                EqualsIntegerSecret => PreprocessingElement::EqualitySecretOutput,
                TruncPr => PreprocessingElement::TruncPr,
                Trunc => PreprocessingElement::Trunc,
                RandomInteger => PreprocessingElement::RandomInteger,
                RandomBoolean => PreprocessingElement::RandomBoolean,
                EcdsaAuxInfo => {
                    auxiliary_materials.insert(AuxiliaryMaterial::Cggmp21AuxiliaryInfo);
                    continue;
                }
            };
            preprocessing_requirements.push(PreprocessingRequirement { element, count: count as u64 });
        }
        let versioned_auxiliary_materials = self
            .dependencies
            .auxiliary_material_metadata_service
            .versions(&Vec::from_iter(auxiliary_materials.iter().cloned()))
            .await
            .map_err(|e| QuoteError::Internal(e.to_string()))?;
        for material in auxiliary_materials {
            if !versioned_auxiliary_materials.contains_key(&material) {
                warn!("Auxiliary material '{material}' is missing");
                return Err(QuoteError::AuxiliaryMaterialMissing);
            }
        }
        let auxiliary_material_requirements = versioned_auxiliary_materials
            .into_iter()
            .map(|(material, version)| AuxiliaryMaterialRequirement { material, version })
            .collect();
        Ok((preprocessing_requirements, auxiliary_material_requirements))
    }

    async fn do_generate_quote(&self, request: PriceQuoteRequest) -> Result<PriceQuote, QuoteError> {
        self.validate_payload_size(&request)?;
        self.validate_program(&request)?;

        let (preprocessing_requirements, auxiliary_material_requirements) = self.analyze_program(&request).await?;
        self.validate_requirements(&preprocessing_requirements)?;

        let token_dollar_price_cents = self.token_price_in_usd_cents().await?;
        let fees = self.calculate_cost(&request, token_dollar_price_cents)?;
        let nonce = random::<[u8; NONCE_LENGTH]>().to_vec();
        let expires_at = self.dependencies.time_service.current_time().add(self.config.quote_ttl);

        let quote = PriceQuote {
            preprocessing_requirements,
            auxiliary_material_requirements,
            request,
            nonce,
            expires_at,
            fees,
        };
        Ok(quote)
    }

    fn convert_requirements(requirements: &[PreprocessingRequirement]) -> MPCProgramRequirements {
        use node_api::preprocessing::rust::PreprocessingElement as Element;
        let mut program_requirements = MPCProgramRequirements::default();
        for requirement in requirements {
            let count = requirement.count as usize;
            program_requirements = match requirement.element {
                Element::Compare => program_requirements.with_compare_elements(count),
                Element::DivisionSecretDivisor => program_requirements.with_division_integer_secret_elements(count),
                Element::EqualitySecretOutput => program_requirements.with_equals_integer_secret_elements(count),
                Element::EqualityPublicOutput => program_requirements.with_public_output_equality_elements(count),
                Element::Modulo => program_requirements.with_modulo_elements(count),
                Element::Trunc => program_requirements.with_trunc_elements(count),
                Element::TruncPr => program_requirements.with_truncpr_elements(count),
                Element::RandomInteger => program_requirements.with_random_integer_elements(count),
                Element::RandomBoolean => program_requirements.with_random_boolean_elements(count),
            }
        }
        program_requirements
    }

    async fn generate_receipt_metadata<'a>(
        &'a self,
        quote: PriceQuote,
        ctx: &mut TransactionContext<'a>,
    ) -> Result<OperationMetadata, PaymentVerificationError> {
        let nonce = ExpireableNonce::new_quote(Nonce(quote.nonce), quote.expires_at);
        let metadata = match quote.request {
            PriceQuoteRequest::PoolStatus => OperationMetadata::PoolStatus,
            PriceQuoteRequest::RetrievePermissions(request) => OperationMetadata::RetrievePermissions(request),
            PriceQuoteRequest::OverwritePermissions(request) => OperationMetadata::OverwritePermissions(request),
            PriceQuoteRequest::UpdatePermissions(request) => OperationMetadata::UpdatePermissions(request),
            PriceQuoteRequest::RetrieveValues(request) => OperationMetadata::RetrieveValues(request),
            PriceQuoteRequest::StoreProgram(request) => OperationMetadata::StoreProgram(request),
            PriceQuoteRequest::StoreValues(request) => OperationMetadata::StoreValues(request),
            PriceQuoteRequest::InvokeCompute(request) => {
                self.generate_invoke_compute_receipt_metadata(
                    quote.preprocessing_requirements,
                    quote.auxiliary_material_requirements,
                    request,
                    &nonce,
                    ctx,
                )
                .await?
            }
        };
        self.dependencies.used_nonces_repo.insert(&nonce, ctx).await?;
        Ok(metadata)
    }

    async fn generate_invoke_compute_receipt_metadata<'a>(
        &'a self,
        preprocessing_requirements: Vec<PreprocessingRequirement>,
        auxiliary_materials_requirements: Vec<AuxiliaryMaterialRequirement>,
        request: InvokeCompute,
        nonce: &ExpireableNonce,
        ctx: &mut TransactionContext<'a>,
    ) -> Result<OperationMetadata, PaymentVerificationError> {
        let amounts =
            preprocessing_requirements.iter().map(|requirement| (requirement.element, requirement.count)).collect();
        let offsets = self.dependencies.offsets_service.request_preprocessing_offsets(amounts, ctx).await?;

        info!("Assigning offsets {offsets:?} to request {}", hex::encode(&nonce.nonce.0));
        let offsets = offsets
            .into_iter()
            .map(|(element, range)| SelectedPreprocessingOffsets {
                element,
                start: range.start,
                end: range.end,
                batch_size: self.config.preprocessing.batch_size(&element),
            })
            .collect();
        let auxiliary_materials = auxiliary_materials_requirements
            .into_iter()
            .map(|r| SelectedAuxiliaryMaterial { material: r.material, version: r.version })
            .collect();
        let metadata = InvokeComputeMetadata { quote: request, offsets, auxiliary_materials };
        let output = OperationMetadata::InvokeCompute(metadata);
        Ok(output)
    }

    async fn token_price_in_usd_cents(&self) -> Result<Decimal, TokenDollarConversionError> {
        let dollar_price = self.dependencies.token_dollar_conversion_service.token_dollar_price().await?;
        dollar_price.checked_mul(Decimal::from(100)).ok_or(TokenDollarConversionError::Internal("Overflow".to_string()))
    }
}

#[async_trait]
impl PaymentService for DefaultPaymentService {
    async fn generate_quote(&self, request: PriceQuoteRequest) -> Result<SignedQuote, QuoteError> {
        let _timer = METRICS.operation_timer("generate_quote");
        let quote = self.do_generate_quote(request).await?;
        let quote = quote.into_proto().encode_to_vec();
        let signature = self.signing_key.sign(&quote);
        Ok(SignedQuote { quote, signature: signature.into() })
    }

    fn verify_decode_quote(&self, signed_quote: SignedQuote) -> Result<PriceQuote, PaymentVerificationError> {
        let _timer = METRICS.operation_timer("verify_decode_quote");
        let signature = Signature::from(signed_quote.signature);
        self.signing_key
            .public_key()
            .verify(&signature, &signed_quote.quote)
            .map_err(|_| PaymentVerificationError::InvalidSignature)?;

        // At this point a failed decode is an internal error because we know we generated this
        // quote.
        PriceQuote::try_decode(&signed_quote.quote)
            .map_err(|_| PaymentVerificationError::Internal("decoding failed".into()))
    }

    async fn verify_payment(
        &self,
        quote: PriceQuote,
        tx_hash: String,
    ) -> Result<OperationMetadata, PaymentVerificationError> {
        let _timer = METRICS.operation_timer("verify_payment");
        let now = self.dependencies.time_service.current_time();
        if quote.expires_at < now {
            info!("Rejecting expired quote: {now} vs {}", quote.expires_at);
            return Err(PaymentVerificationError::QuoteExpired);
        }
        let tx = self.fetch_transaction(&tx_hash).await?;

        if tx.nonce.0 != quote.nonce {
            METRICS.inc_invalid_txs("invalid nonce");
            warn!("Quote and tx nonce mismatch: {} {}", hex::encode(&quote.nonce), hex::encode(&tx.nonce.0));
            return Err(PaymentVerificationError::NonceMismatch);
        } else if tx.paid_amount < quote.fees.tokens {
            METRICS.inc_invalid_txs("insufficient_payment");
            warn!("Quoted {} but tx.paid_amount is {}", quote.fees.tokens, tx.paid_amount);
            return Err(PaymentVerificationError::InsufficientPayment);
        } else if tx.paid_amount > quote.fees.tokens {
            warn!(
                "User paid more than expected: quote asked for {} but {} was paid",
                quote.fees.tokens, tx.paid_amount
            );
        }
        let mut ctx = self.dependencies.balance_repo.begin_transaction().await?;
        let metadata = self.generate_receipt_metadata(quote, &mut ctx).await?;
        ctx.commit().await.map_err(|e| PaymentVerificationError::Internal(e.to_string()))?;
        Ok(metadata)
    }

    async fn deduct_payment_from_balance(
        &self,
        quote: PriceQuote,
        user_id: &UserId,
    ) -> Result<OperationMetadata, PaymentVerificationError> {
        let _timer = METRICS.operation_timer("deduct_payment_from_balance");
        let paid_amount = i64::try_from(quote.fees.credits)
            .map_err(|_| PaymentVerificationError::Internal("funds overflow".to_string()))?;
        let mut ctx = self.dependencies.balance_repo.begin_transaction().await?;
        // This performs the validation that the user has enough funds internally
        self.dependencies.balance_repo.remove_funds(user_id, paid_amount, &mut ctx).await?;
        info!("Removed {paid_amount} from funds for user {user_id}");
        let metadata = self.generate_receipt_metadata(quote, &mut ctx).await?;
        ctx.commit().await.map_err(|e| PaymentVerificationError::Internal(e.to_string()))?;
        Ok(metadata)
    }

    fn generate_payment_receipt(
        &self,
        identifier: Vec<u8>,
        metadata: OperationMetadata,
    ) -> Result<SignedReceipt, InvalidSignature> {
        let _timer = METRICS.operation_timer("generate_payment_receipt");
        let expires_at = self.dependencies.time_service.current_time().add(self.config.receipt_ttl);
        let receipt = Receipt { identifier, metadata, expires_at };
        let receipt = receipt.into_proto().encode_to_vec();
        let signature = self.signing_key.sign(&receipt).into();
        Ok(SignedReceipt { signature, receipt })
    }

    async fn account_balance(&self, user: &UserId) -> Result<AccountBalance, BalanceLookupError> {
        let _timer = METRICS.operation_timer("account_balance");
        let result = self
            .dependencies
            .balance_repo
            .find(user, &mut Default::default())
            .await
            .map_err(|e| BalanceLookupError(e.to_string()))?;
        let balance =
            result.unwrap_or_else(|| AccountBalance { account: *user, balance: 0, updated_at: Default::default() });
        Ok(balance)
    }

    async fn add_funds(&self, request: AddFundsRequest) -> Result<(), AddFundsError> {
        let _timer = METRICS.operation_timer("add_funds");
        let AddFundsRequest { payload, tx_hash } = request;
        let decoded_payload = AddFundsPayload::try_decode(&payload).map_err(|_| AddFundsError::InvalidPayload)?;
        let transaction = self.fetch_transaction(&tx_hash).await?;
        let payload_hash = Sha256::digest(&payload);
        if transaction.nonce.0 != payload_hash.as_slice() {
            return Err(AddFundsError::HashMismatch);
        }
        let recipient = decoded_payload.recipient;
        let minimum = self.minimum_add_funds_payment().await.map_err(|e| AddFundsError::Internal(e.to_string()))?;
        // Give a little leeway since the minimum payment amount may have changed in between the
        // user looking it up and them paying.
        let minimum = TokenAmount::Unil((minimum.to_unil() as f64 * MINIMUM_PAYMENT_THRESHOLD) as u64);

        let paid_amount = TokenAmount::Unil(transaction.paid_amount);
        if paid_amount < minimum {
            return Err(AddFundsError::PaymentTooSmall);
        }

        let token_dollar_price_usd_cents = self.token_price_in_usd_cents().await?;

        let paid_amount_nil = Decimal::from(transaction.paid_amount)
            .checked_div(Decimal::from(1_000_000))
            .ok_or(AddFundsError::Internal("division error".to_string()))?;

        let paid_amount_usd_cents = paid_amount_nil
            .checked_mul(token_dollar_price_usd_cents)
            .ok_or(AddFundsError::Internal("overflow".to_string()))?;

        let paid_amount =
            paid_amount_usd_cents.ceil().to_i64().ok_or(AddFundsError::Internal("conversion error".to_string()))?;
        info!("Adding {paid_amount} credits to account {recipient}, paid for in tx {tx_hash}");

        let transfer = Transfer { tx_hash, account: recipient, amount: paid_amount };
        let mut ctx = self.dependencies.balance_repo.begin_transaction().await?;
        self.dependencies.transfers_repo.insert(transfer, &mut ctx).await?;
        self.dependencies.balance_repo.add_funds(&recipient, paid_amount, &mut ctx).await?;
        ctx.commit().await.map_err(|e| AddFundsError::Internal(format!("failed to commit tx: {e}")))?;
        Ok(())
    }

    async fn minimum_add_funds_payment(&self) -> Result<TokenAmount, MinimumAddFundsPaymentError> {
        let nil_price_usd_cents =
            self.token_price_in_usd_cents().await.map_err(|e| MinimumAddFundsPaymentError(e.to_string()))?;
        let minimum_tokens_unil = self
            .config
            .minimum_add_funds_credits
            .checked_mul(Decimal::from(1_000_000))
            .ok_or(MinimumAddFundsPaymentError("overflow".into()))?
            .checked_div(nil_price_usd_cents)
            .ok_or(MinimumAddFundsPaymentError("division error".to_string()))?;
        let minimum_tokens_unil =
            minimum_tokens_unil.to_u64().ok_or(MinimumAddFundsPaymentError("Conversion error".to_string()))?;
        Ok(TokenAmount::Unil(minimum_tokens_unil))
    }

    async fn nil_credits_conversion_rate(&self) -> Result<u64, ConversionRateError> {
        let price = self.token_price_in_usd_cents().await.map_err(|e| ConversionRateError(e.to_string()))?;
        price.try_into().map_err(|_| ConversionRateError("overflow".into()))
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Transaction {
    pub(crate) nonce: Nonce,
    pub(crate) paid_amount: u64,
}

#[derive(Clone, Debug, PartialEq, thiserror::Error)]
pub(crate) enum TransactionFetchError {
    #[error("internal: {0}")]
    Internal(String),

    #[error("transaction not committed yet")]
    NotCommitted,

    #[error("transaction not found")]
    NotFound,
}

struct Metrics {
    operation_duration: MaybeMetric<Histogram<Duration>>,
    invalid_transactions: MaybeMetric<Counter>,
    transaction_fetch_errors: MaybeMetric<Counter>,
}

impl Default for Metrics {
    fn default() -> Self {
        let operation_duration = Histogram::new(
            "payments_operation_duration_seconds",
            "Duration of payment operations in seconds",
            &["operation"],
            TimingBuckets::sub_second(),
        )
        .into();
        let invalid_transactions =
            Counter::new("invalid_tx_total", "Number of invalid nilchain transactions found by reason", &["reason"])
                .into();
        let transaction_fetch_errors =
            Counter::new("transaction_fetch_errors_total", "Number of errors found when fetching transactions", &[])
                .into();
        Self { operation_duration, invalid_transactions, transaction_fetch_errors }
    }
}

impl Metrics {
    fn operation_timer(&self, operation: &str) -> ScopedTimer<impl SingleHistogramMetric<Duration>> {
        self.operation_duration.with_labels([("operation", operation)]).into_timer()
    }

    fn inc_invalid_txs(&self, reason: &str) {
        self.invalid_transactions.with_labels([("reason", reason)]).inc();
    }

    fn inc_transaction_fetch_errors(&self) {
        self.transaction_fetch_errors.with_labels([]).inc();
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::{
        services::{
            auxiliary_material::MockAuxiliaryMaterialMetadataService, offsets::MockElementOffsetsService,
            programs::MockProgramService, time::MockTimeService,
            token_dollar_conversion::MockTokenDollarConversionService,
        },
        storage::repositories::{
            balances::{AccountBalance, MockAccountBalanceRepository},
            nonces::MockUsedNoncesRepository,
            transfers::MockTransfersRepository,
        },
    };
    use chrono::Utc;
    use mockall::predicate::{always, eq};
    use nillion_chain_client::{
        transactions::TokenAmount,
        tx::{PaymentTransaction, RetrieveError},
    };
    use node_api::payments::rust::InvokeCompute;
    use node_config::PreprocessingProtocolConfig;
    use rstest::rstest;
    use rust_decimal::prelude::FromPrimitive;
    use std::{
        collections::{BTreeMap, HashMap},
        sync::Mutex,
    };
    use test_programs::PROGRAMS;
    use tracing_test::traced_test;

    mockall::mock! {
        PaymentTransactionRetriever {}

        #[async_trait]
        impl PaymentTransactionRetriever for PaymentTransactionRetriever {
            async fn get(&self, tx_hash: &str) -> Result<PaymentTransaction, RetrieveError>;
        }
    }

    fn default_pricing_config() -> PricingConfig {
        PricingConfig {
            retrieve_permissions_price: 1,
            pool_status_price: 1,
            overwrite_permissions_price: 1,
            update_permissions_price: 1,
            retrieve_values_price: 1,
            store_program_price: 1,
            store_values_price: 1,
            invoke_compute_price: 1,
        }
    }

    fn default_quote() -> PriceQuote {
        PriceQuote {
            preprocessing_requirements: Default::default(),
            auxiliary_material_requirements: Default::default(),
            request: PriceQuoteRequest::PoolStatus,
            nonce: Default::default(),
            expires_at: Utc::now(),
            fees: QuoteFees { tokens: 1, credits: 1 },
        }
    }

    struct ServiceBuilder {
        signing_key: SigningKey,
        config: PaymentsServiceConfig,
        time_service: MockTimeService,
        programs_service: MockProgramService,
        balance_repo: MockAccountBalanceRepository,
        transfers_repo: MockTransfersRepository,
        tx_retriever: MockPaymentTransactionRetriever,
        auxiliary_material_metadata_service: MockAuxiliaryMaterialMetadataService,
        used_nonces_repo: MockUsedNoncesRepository,
        offsets_service: MockElementOffsetsService,
        token_dollar_conversion_service: MockTokenDollarConversionService,
    }

    impl ServiceBuilder {
        fn build(self) -> DefaultPaymentService {
            DefaultPaymentService::new(
                self.signing_key,
                PaymentServiceDependencies {
                    time_service: Arc::new(self.time_service),
                    programs_service: Arc::new(self.programs_service),
                    balance_repo: Arc::new(self.balance_repo),
                    transfers_repo: Arc::new(self.transfers_repo),
                    used_nonces_repo: Arc::new(self.used_nonces_repo),
                    tx_retriever: Arc::new(self.tx_retriever),
                    offsets_service: Arc::new(self.offsets_service),
                    auxiliary_material_metadata_service: Arc::new(self.auxiliary_material_metadata_service),
                    token_dollar_conversion_service: Arc::new(self.token_dollar_conversion_service),
                },
                self.config,
            )
        }
    }

    impl Default for ServiceBuilder {
        fn default() -> Self {
            let mut time_service = MockTimeService::default();
            time_service.expect_current_time().returning(|| Utc::now());

            let mut balance_repo = MockAccountBalanceRepository::default();
            balance_repo.expect_begin_transaction().returning(|| Ok(Default::default()));

            Self {
                signing_key: SigningKey::generate_secp256k1(),
                config: make_config(),
                time_service,
                programs_service: Default::default(),
                balance_repo,
                transfers_repo: Default::default(),
                tx_retriever: Default::default(),
                auxiliary_material_metadata_service: Default::default(),
                used_nonces_repo: Default::default(),
                offsets_service: Default::default(),
                token_dollar_conversion_service: Default::default(),
            }
        }
    }

    fn make_config() -> PaymentsServiceConfig {
        PaymentsServiceConfig {
            max_payload_size: 128,
            pricing: default_pricing_config(),
            preprocessing: PreprocessingConfig::new(PreprocessingProtocolConfig {
                batch_size: 1,
                generation_threshold: 64,
                target_offset_jump: 1,
            }),
            quote_ttl: Duration::from_secs(60),
            receipt_ttl: Duration::from_secs(60),
            minimum_add_funds_credits: 1.into(),
        }
    }

    #[tokio::test]
    async fn transaction_fetching() {
        let now = Utc::now();
        let signing_key = SigningKey::generate_secp256k1();
        let mut time_service = MockTimeService::default();
        time_service.expect_current_time().return_once(move || now);

        let tx_hash = "my-hash";

        let tx =
            PaymentTransaction { resource: b"123".into(), from_address: "".to_string(), amount: TokenAmount::Unil(42) };
        let mut tx_retriever = MockPaymentTransactionRetriever::default();
        tx_retriever.expect_get().with(eq(tx_hash)).return_once(|_| Ok(tx));

        let service = ServiceBuilder { signing_key, time_service, tx_retriever, ..Default::default() }.build();
        let tx = service.fetch_transaction(tx_hash).await.expect("fetching tx failed");
        assert_eq!(tx.nonce.0, b"123");
        assert_eq!(tx.paid_amount, 42);
    }

    #[rstest]
    #[case::not_committed(RetrieveError::NotCommitted, TransactionFetchError::NotCommitted)]
    #[tokio::test]
    async fn transaction_fetching_not_committed(
        #[case] error: RetrieveError,
        #[case] returned_error: TransactionFetchError,
    ) {
        let signing_key = SigningKey::generate_secp256k1();

        let tx_hash = "my-hash";
        let mut tx_retriever = MockPaymentTransactionRetriever::default();
        tx_retriever.expect_get().with(eq(tx_hash)).return_once(move |_| Err(error));

        let service = ServiceBuilder { signing_key, tx_retriever, ..Default::default() }.build();
        let result = service.fetch_transaction(tx_hash).await;
        assert_eq!(result, Err(returned_error));
    }

    #[test]
    #[traced_test]
    fn decode_quote() {
        let signing_key = SigningKey::generate_secp256k1();
        let quote = default_quote().into_proto().encode_to_vec();
        let signature = signing_key.sign(&quote).into();
        let signed_quote = SignedQuote { quote, signature };

        let service = ServiceBuilder { signing_key, ..Default::default() }.build();
        service.verify_decode_quote(signed_quote).expect("invalid quote");
    }

    #[test]
    #[traced_test]
    fn decode_invalid_quote() {
        let signing_key = SigningKey::generate_secp256k1();
        let quote = default_quote().into_proto().encode_to_vec();
        let mut signature: Vec<_> = signing_key.sign(&quote).into();
        // break the signature
        signature[0] = signature[0].wrapping_add(1);
        let signed_quote = SignedQuote { quote, signature };

        let service = ServiceBuilder { signing_key, ..Default::default() }.build();
        service.verify_decode_quote(signed_quote).expect_err("signature validation succeeded");
    }

    #[tokio::test]
    #[traced_test]
    async fn tx_nonce_mismatch() {
        let now = Utc::now();
        let signing_key = SigningKey::generate_secp256k1();
        let mut quote = default_quote();
        quote.fees.tokens = 42;
        quote.expires_at = now.add(Duration::from_secs(5));
        let tx_hash = "my-hash".to_string();
        let tx = PaymentTransaction {
            resource: b"txs-nonce".into(),
            from_address: "".to_string(),
            amount: TokenAmount::Unil(quote.fees.tokens),
        };

        let mut time_service = MockTimeService::default();
        time_service.expect_current_time().return_once(move || now);

        let mut tx_retriever = MockPaymentTransactionRetriever::default();
        tx_retriever.expect_get().with(eq(tx_hash.clone())).return_once(|_| Ok(tx));

        let service = ServiceBuilder { signing_key, time_service, tx_retriever, ..Default::default() }.build();
        let result = service.verify_payment(quote, tx_hash).await;
        assert!(matches!(result, Err(PaymentVerificationError::NonceMismatch)));
    }

    #[tokio::test]
    #[traced_test]
    async fn reject_underpayment() {
        let now = Utc::now();
        let signing_key = SigningKey::generate_secp256k1();
        let nonce = Nonce(b"123".into());
        let mut quote = default_quote();
        quote.nonce = nonce.0.clone();
        quote.fees.tokens = 100;
        quote.expires_at = now.add(Duration::from_secs(5));

        let tx_hash = "my-hash".to_string();
        let tx = PaymentTransaction {
            resource: nonce.0.clone(),
            from_address: "".to_string(),
            amount: TokenAmount::Unil(quote.fees.tokens - 1),
        };

        let mut time_service = MockTimeService::default();
        time_service.expect_current_time().return_once(move || now);

        let mut tx_retriever = MockPaymentTransactionRetriever::default();
        tx_retriever.expect_get().with(eq(tx_hash.clone())).return_once(|_| Ok(tx));

        let service = ServiceBuilder { signing_key, time_service, tx_retriever, ..Default::default() }.build();

        let result = service.verify_payment(quote, tx_hash).await;
        assert!(matches!(result, Err(PaymentVerificationError::InsufficientPayment)));
    }

    #[tokio::test]
    async fn quote_compute() {
        let program_requirements =
            MPCProgramRequirements::default().with_division_integer_secret_elements(42).with_ecdsa_aux_info();
        let program_id = ProgramId::Builtin("foo".to_string());
        let program = PROGRAMS.program("simple").unwrap().0;

        let mut programs_service = MockProgramService::default();
        programs_service.expect_find().with(eq(program_id.clone())).return_once(move |_| Ok(program));
        programs_service.expect_requirements().returning(move |_| Ok(program_requirements.clone()));

        let mut auxiliary_material_metadata_service = MockAuxiliaryMaterialMetadataService::default();
        auxiliary_material_metadata_service
            .expect_versions()
            .with(eq([AuxiliaryMaterial::Cggmp21AuxiliaryInfo]))
            .return_once(|_| Ok(HashMap::from([(AuxiliaryMaterial::Cggmp21AuxiliaryInfo, 1337)])));

        let request = PriceQuoteRequest::InvokeCompute(InvokeCompute {
            program_id: program_id.to_string(),
            values_payload_size: 0,
        });

        let mut token_dollar_conversion_service = MockTokenDollarConversionService::default();
        token_dollar_conversion_service.expect_token_dollar_price().return_once(|| Ok(Decimal::from(1)));

        let service = ServiceBuilder {
            programs_service,
            auxiliary_material_metadata_service,
            token_dollar_conversion_service,
            ..Default::default()
        }
        .build();
        let quote = service.generate_quote(request).await.expect("quoting failed");
        let quote = PriceQuote::try_decode(&quote.quote).expect("invalid quote");

        let preprocessing_requirements =
            vec![PreprocessingRequirement { element: PreprocessingElement::DivisionSecretDivisor, count: 42 }];
        let auxiliary_material_requirements =
            vec![AuxiliaryMaterialRequirement { material: AuxiliaryMaterial::Cggmp21AuxiliaryInfo, version: 1337 }];
        assert_eq!(quote.preprocessing_requirements, preprocessing_requirements);
        assert_eq!(quote.auxiliary_material_requirements, auxiliary_material_requirements);
    }

    #[tokio::test]
    async fn compute_receipt() {
        let mut builder = ServiceBuilder::default();
        let preprocessing_requirements =
            vec![PreprocessingRequirement { element: PreprocessingElement::Compare, count: 100 }];
        let auxiliary_material_requirements =
            vec![AuxiliaryMaterialRequirement { material: AuxiliaryMaterial::Cggmp21AuxiliaryInfo, version: 42 }];
        let input_offsets = BTreeMap::from([(PreprocessingElement::Compare, 42..142)]);
        let offsets = vec![SelectedPreprocessingOffsets {
            element: PreprocessingElement::Compare,
            start: 42,
            end: 142,
            batch_size: 1,
        }];
        let selected_auxiliary_materials =
            vec![SelectedAuxiliaryMaterial { material: AuxiliaryMaterial::Cggmp21AuxiliaryInfo, version: 42 }];
        let request = InvokeCompute { program_id: "foo".into(), values_payload_size: 0 };
        builder
            .offsets_service
            .expect_request_preprocessing_offsets()
            .with(eq(vec![(PreprocessingElement::Compare, 100u64)]), always())
            .return_once(move |_, _| Ok(input_offsets));

        let metadata = builder
            .build()
            .generate_invoke_compute_receipt_metadata(
                preprocessing_requirements,
                auxiliary_material_requirements,
                request,
                &ExpireableNonce::new_quote(Nonce(vec![1]), Utc::now()),
                &mut Default::default(),
            )
            .await
            .expect("generating receipt failed");
        let OperationMetadata::InvokeCompute(metadata) = metadata else {
            panic!("not an invoke compute");
        };
        assert_eq!(metadata.offsets, offsets);
        assert_eq!(metadata.auxiliary_materials, selected_auxiliary_materials);
    }

    #[tokio::test]
    async fn generate_quote() {
        let request = PriceQuoteRequest::PoolStatus;
        let nonce = Arc::new(Mutex::new(vec![]));
        let cost = Arc::new(Mutex::new(0));
        let mut builder = ServiceBuilder::default();
        // this is kind of crappy but we need the verify payment's tx lookup to use properties of
        // the transaction so we need to populate these later on.
        {
            let nonce = nonce.clone();
            let cost = cost.clone();
            builder.tx_retriever.expect_get().return_once(move |_| {
                Ok(PaymentTransaction {
                    resource: nonce.lock().unwrap().clone(),
                    from_address: "".into(),
                    amount: TokenAmount::Unil(*cost.lock().unwrap()),
                })
            });
        }

        builder.token_dollar_conversion_service.expect_token_dollar_price().return_once(|| Ok(Decimal::from(1)));

        builder.used_nonces_repo.expect_insert().return_once(|_, _| Ok(()));

        let service = builder.build();
        let signed_quote = service.generate_quote(request).await.expect("quoting failed");
        let quote = PriceQuote::try_decode(&signed_quote.quote).expect("invalid encoding");
        // now populate these for the next call.
        *nonce.lock().unwrap() = quote.nonce.clone();
        *cost.lock().unwrap() = quote.fees.tokens;

        service.verify_payment(quote, "".into()).await.expect("verification failed");
    }

    #[tokio::test]
    async fn deduct_payment_from_balance() {
        let mut builder = ServiceBuilder::default();
        let quote = PriceQuote {
            nonce: vec![1],
            fees: QuoteFees { credits: 42, ..Default::default() },
            request: PriceQuoteRequest::PoolStatus,
            expires_at: Utc::now(),
            preprocessing_requirements: vec![],
            auxiliary_material_requirements: vec![],
        };
        let user_id = UserId::from_bytes(b"bob");
        builder.balance_repo.expect_find().with(eq(user_id), always()).return_once(move |_, _| {
            Ok(Some(AccountBalance { account: user_id, balance: 42, updated_at: Default::default() }))
        });
        builder
            .balance_repo
            .expect_remove_funds()
            .with(eq(user_id), eq(42), always())
            .return_once(|_, _, _| Ok(Default::default()));
        builder
            .used_nonces_repo
            .expect_insert()
            .with(eq(ExpireableNonce::new_quote(Nonce(quote.nonce.clone()), quote.expires_at.clone())), always())
            .return_once(|_, _| Ok(()));
        builder.build().deduct_payment_from_balance(quote, &user_id).await.expect("deduct failed");
    }

    #[tokio::test]
    async fn add_funds() {
        let recipient = UserId::from_bytes(b"mike");
        let amount = TokenAmount::Nil(42);
        let payload = AddFundsPayload { recipient, nonce: random() };
        let request = AddFundsRequest { payload: payload.into_proto().encode_to_vec(), tx_hash: "hash".into() };
        let hash = Sha256::digest(&request.payload).to_vec();

        let mut builder = ServiceBuilder::default();
        builder.token_dollar_conversion_service.expect_token_dollar_price().returning(|| Ok(Decimal::from(1)));
        builder
            .tx_retriever
            .expect_get()
            .with(eq(request.tx_hash.clone()))
            .return_once(move |_| Ok(PaymentTransaction { resource: hash, from_address: "foo".into(), amount }));
        builder.balance_repo.expect_begin_transaction().return_once(move || Ok(Default::default()));
        builder
            .balance_repo
            .expect_add_funds()
            .with(eq(recipient), eq(42 * 100), always())
            .return_once(move |_, _, _| Ok(()));
        let expected_transfer = Transfer { tx_hash: request.tx_hash.clone(), account: recipient, amount: 42 * 100 };
        builder.transfers_repo.expect_insert().with(eq(expected_transfer), always()).return_once(|_, _| Ok(()));
        builder.build().add_funds(request).await.expect("adding funds failed");
    }

    #[tokio::test]
    async fn add_funds_invalid_hash() {
        let payload = AddFundsPayload { recipient: UserId::from_bytes(b"mike"), nonce: random() };
        let request = AddFundsRequest { payload: payload.into_proto().encode_to_vec(), tx_hash: "hash".into() };
        let hash = b"nope".to_vec();

        let mut builder = ServiceBuilder::default();
        builder.tx_retriever.expect_get().with(eq(request.tx_hash.clone())).return_once(move |_| {
            Ok(PaymentTransaction { resource: hash, from_address: "foo".into(), amount: TokenAmount::Nil(42) })
        });
        let err = builder.build().add_funds(request).await.expect_err("adding funds succeeded");
        assert!(matches!(err, AddFundsError::HashMismatch), "{err}");
    }

    #[tokio::test]
    async fn add_funds_small_payment() {
        let payload = AddFundsPayload { recipient: UserId::from_bytes(b"mike"), nonce: random() };
        let request = AddFundsRequest { payload: payload.into_proto().encode_to_vec(), tx_hash: "tx_hash".into() };
        let hash = Sha256::digest(&request.payload).to_vec();

        let mut builder = ServiceBuilder::default();

        builder.config.minimum_add_funds_credits = 2.into();
        builder.tx_retriever.expect_get().with(eq(request.tx_hash.clone())).return_once(move |_| {
            Ok(PaymentTransaction { resource: hash, from_address: "foo".into(), amount: TokenAmount::Unil(1) })
        });
        builder
            .token_dollar_conversion_service
            .expect_token_dollar_price()
            .returning(|| Ok(Decimal::from_f64(0.001).unwrap()));
        let service = builder.build();
        let err = service.add_funds(request).await.expect_err("adding funds succeeded");
        assert!(matches!(err, AddFundsError::PaymentTooSmall), "{err}");
    }

    #[tokio::test]
    async fn minimum_payment() {
        let mut builder = ServiceBuilder::default();
        // 25 cents minimum
        builder.config.minimum_add_funds_credits = 25.into();
        // token is 2 dollars
        builder.token_dollar_conversion_service.expect_token_dollar_price().returning(|| Ok(Decimal::from(2)));

        let service = builder.build();
        let minimum = service.minimum_add_funds_payment().await.expect("failed to get minimum");
        // 1 token is 2 dollars, so the minimum (25 cents) is 1 token / 8 == 125_000 unil
        assert_eq!(minimum, TokenAmount::Unil(125_000));
    }
}
