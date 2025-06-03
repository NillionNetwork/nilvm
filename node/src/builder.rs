//! Components in the node.

use crate::{
    channels::{ClusterChannels, DefaultClusterChannels},
    config::ObjectStorageConfig,
    controllers::{
        compute::{ComputeApi, ComputeApiHandles, ComputeApiServices},
        leader_queries::{LeaderQueriesApi, LeaderQueriesApiServices},
        membership::MembershipApi,
        payments::{PaymentsApi, PaymentsApiServices},
        permissions::{PermissionsApi, PermissionsApiServices},
        preprocessing::{PreprocessingApi, PreprocessingApiServices},
        programs::{ProgramsApi, ProgramsApiServices},
        values::{ValuesApi, ValuesApiServices},
    },
    grpc::{
        interceptors::{InternalServiceInterceptor, RateLimitInterceptor},
        metrics::MetricsMiddleware,
    },
    observability::{PrometheusExporter, process::ProcessMetricsCollector},
    services::{
        auxiliary_material::{
            AuxiliaryMaterialMetadataService, AuxiliaryMaterialService, DefaultAuxiliaryMaterialMetadataService,
            DefaultAuxiliaryMaterialService,
        },
        blob::DefaultBlobService,
        nonce::{DefaultNonceService, NonceService},
        offsets::{DefaultElementOffsetsService, ElementOffsetsService},
        payments::{DefaultPaymentService, PaymentService, PaymentServiceDependencies, PaymentsServiceConfig},
        preprocessing::{DefaultPreprocessingBlobService, PreprocessingBlobService},
        programs::{DefaultProgramService, ProgramService},
        receipts::{DefaultReceiptsService, ReceiptsService},
        results::{DefaultResultsService, ResultsService},
        runtime_elements::DefaultRuntimeElementsService,
        scheduling::{DefaultPreprocessingSchedulingService, PreprocessingSchedulingService},
        time::{DefaultTimeService, TimeService},
        token_dollar_conversion::{
            HardcodedTokenDollarConversionService, TokenDollarConversionCoinGeckoService, TokenDollarConversionService,
        },
        user_values::{DefaultUserValuesService, UserValuesService},
        uuid::DefaultUuidService,
    },
    stateful::{
        auxiliary_material_scheduler::AuxiliaryMaterialScheduler,
        builder::{DefaultPrimeBuilder, PrimeBuilder},
        cleanup::{
            ExpiredValuesCleanup, NonceCleanup, UsedPreprocessingCleanup, balances::BalancesCleanup,
            compute_results::ExpiredComputeResultsCleanup,
        },
        preprocessing_scheduler::{PreprocessingScheduler, PreprocessingSchedulerServices},
    },
    storage::{
        metrics::{MetricsExporterRepository, StorageMetricsExporter},
        repositories::{
            auxiliary_material_meta::{AuxiliaryMaterialMetadataRepository, SqliteAuxiliaryMaterialMetadataRepository},
            balances::{AccountBalanceRepository, SqliteAccountBalanceRepository},
            blob::{
                BinarySerde, BlobRepository, FilesystemBlobRepository, MemoryBlobRepository, ObjectStoreRepository,
            },
            blob_expirations::SqliteBlobExpirationsRepository,
            nonces::{SqliteUsedNoncesRepository, UsedNoncesRepository},
            offsets::{PreprocessingOffsetsRepository, SqlitePreprocessingOffsetsRepository},
            transfers::SqliteTransfersRepository,
        },
        sqlite::SqliteDb,
    },
};
use anyhow::{Context, Error, anyhow};
use basic_types::PartyId;
use chrono::Days;
use futures::executor::block_on;
use governor::Quota;
use grpc_channel::auth::ServerAuthInterceptor;
use math_lib::modular::{EncodedModularNumber, U64SafePrime, U128SafePrime, U256SafePrime};
use mpc_vm::vm::ExecutionVmConfig;
use nilchain_client::tx::{DefaultPaymentTransactionRetriever, PaymentTransactionRetriever};
use node_api::{
    auth::rust::{PublicKey, UserId},
    compute::proto::compute_server::ComputeServer,
    leader_queries::proto::leader_queries_server::LeaderQueriesServer,
    membership::{
        proto::membership_server::MembershipServer,
        rust::{Cluster, ClusterMember, NodeId, Prime},
    },
    payments::proto::payments_server::PaymentsServer,
    permissions::proto::permissions_server::PermissionsServer,
    preprocessing::{proto::preprocessing_server::PreprocessingServer, rust::PreprocessingElement},
    programs::proto::programs_server::ProgramsServer,
    values::proto::values_server::ValuesServer,
};
use node_config::{
    AuxiliaryMaterialConfig, KeyKind, MetricsConfig, PaymentsConfig, PrefundedAccount, PreprocessingConfig,
    PrivateKeyConfig, RateLimitBucket,
};
use object_store::{
    ClientOptions,
    aws::{AmazonS3, AmazonS3Builder, AmazonS3ConfigKey, S3ConditionalPut, resolve_bucket_region},
};
use program_auditor::ProgramAuditor;
use protocols::{
    conditionals::{
        equality::offline::EncodedPrepPrivateOutputEqualityShares,
        equality_public_output::offline::EncodedPrepPublicOutputEqualityShares,
        less_than::offline::EncodedPrepCompareShares,
    },
    division::{
        division_secret_divisor::offline::EncodedPrepDivisionIntegerSecretShares,
        modulo_public_divisor::offline::EncodedPrepModuloShares,
        modulo2m_public_divisor::offline::EncodedPrepModulo2mShares,
        truncation_probabilistic::offline::EncodedPrepTruncPrShares,
    },
    random::random_bit::EncodedBitShare,
    threshold_ecdsa::auxiliary_information::output::EcdsaAuxInfo,
};
use rust_decimal::{Decimal, prelude::FromPrimitive};
use serde::{Serialize, de::DeserializeOwned};
use shamir_sharing::secret_sharer::ShamirSecretSharer;
use std::{collections::HashMap, fs, path::PathBuf, sync::Arc, time::Duration};
use strum::IntoEnumIterator;
use tokio::{sync::oneshot, task::JoinHandle, time::timeout};
use tokio_util::sync::CancellationToken;
use tonic::{
    codegen::InterceptedService,
    transport::{Identity, ServerTlsConfig},
};
use tonic_health::{ServingStatus, pb::FILE_DESCRIPTOR_SET as TONIC_HEALTH_DESCRIPTOR, server::health_reporter};
use tonic_middleware::MiddlewareLayer;
use tonic_reflection::server::Builder as ReflectionBuilder;
use tonic_web::GrpcWebLayer;
use tower_http::cors::CorsLayer;
use tracing::{error, info, warn};
use user_keypair::{
    SigningKey,
    ed25519::{Ed25519PublicKey, Ed25519SigningKey},
    secp256k1::{Secp256k1PublicKey, Secp256k1SigningKey},
};

const GRACEFUL_SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(300);

const S3_OPERATION_TIMEOUT: Duration = Duration::from_secs(30);
const S3_CONNECT_TIMEOUT: Duration = Duration::from_secs(5);

struct LeaderDependencies {
    payments: Arc<dyn PaymentService>,
    offsets: Arc<dyn ElementOffsetsService>,
    auxiliary_material_metadata_repository: Arc<dyn AuxiliaryMaterialMetadataRepository>,
    auxiliary_material_metadata_service: Arc<dyn AuxiliaryMaterialMetadataService>,
    preprocessing_config: PreprocessingConfig,
    auxiliary_material_config: AuxiliaryMaterialConfig,
    account_balances: Arc<dyn AccountBalanceRepository>,
}

struct Dependencies {
    prep_compare: Arc<dyn PreprocessingBlobService<EncodedPrepCompareShares>>,
    prep_division_integer_secret: Arc<dyn PreprocessingBlobService<EncodedPrepDivisionIntegerSecretShares>>,
    prep_modulo: Arc<dyn PreprocessingBlobService<EncodedPrepModuloShares>>,
    prep_public_output_equality: Arc<dyn PreprocessingBlobService<EncodedPrepPublicOutputEqualityShares>>,
    prep_equals_integer_secret: Arc<dyn PreprocessingBlobService<EncodedPrepPrivateOutputEqualityShares>>,
    prep_truncpr: Arc<dyn PreprocessingBlobService<EncodedPrepTruncPrShares>>,
    prep_trunc: Arc<dyn PreprocessingBlobService<EncodedPrepModulo2mShares>>,
    random_integer: Arc<dyn PreprocessingBlobService<EncodedModularNumber>>,
    random_boolean: Arc<dyn PreprocessingBlobService<EncodedBitShare>>,
    cggmp21_aux_info: Arc<dyn AuxiliaryMaterialService<EcdsaAuxInfo>>,
    user_values: Arc<dyn UserValuesService>,
    results: Arc<dyn ResultsService>,
    time: Arc<dyn TimeService>,
    programs: Arc<dyn ProgramService>,
    tx_retriever: Arc<dyn PaymentTransactionRetriever>,
    receipts: Arc<dyn ReceiptsService>,
    nonces: Arc<dyn NonceService>,
    nonces_repository: Arc<dyn UsedNoncesRepository>,
    token_dollar_conversion_service: Arc<dyn TokenDollarConversionService>,
    leader: Option<LeaderDependencies>,
    sqlite: SqliteDb,
    sqlite_repositories: Vec<MetricsExporterRepository>,
    compute_api_handles: ComputeApiHandles,
    channels: Arc<dyn ClusterChannels>,
    cluster: Cluster,
    cancel_token: CancellationToken,
}

/// The mode used for preprocessing.
#[derive(Default, Clone, Debug)]
pub enum PreprocessingMode {
    /// The real mode.
    #[default]
    Real,

    /// A fake mode where shares are hardcoded. **This should only be used for testing**.
    Fake,
}

/// A helper to construct the various node components.
pub struct NodeBuilder {
    config: node_config::Config,
    preprocessing_mode: PreprocessingMode,
}

impl NodeBuilder {
    /// Construct a new node builder for the given config.
    pub fn new(config: node_config::Config) -> Self {
        Self { config, preprocessing_mode: PreprocessingMode::default() }
    }

    /// Configure the preprocessing mode to use.
    pub fn preprocessing_mode(mut self, mode: PreprocessingMode) -> Self {
        self.preprocessing_mode = mode;
        self
    }

    /// Build and launch the node.
    pub fn launch(self) -> anyhow::Result<NodeHandle> {
        let Self { config, preprocessing_mode } = self;
        let signing_key: SigningKey = match &config.identity.private_key {
            PrivateKeyConfig::Seed { seed, kind } => match kind {
                KeyKind::Ed25519 => Ed25519SigningKey::from_seed(seed).into(),
                KeyKind::Secp256k1 => Secp256k1SigningKey::try_from_seed(seed)?.into(),
            },
            PrivateKeyConfig::Raw { key, kind } => match kind {
                KeyKind::Ed25519 => Ed25519SigningKey::try_from(key.as_ref())?.into(),
                KeyKind::Secp256k1 => Secp256k1SigningKey::try_from(key.as_ref())?.into(),
            },
            PrivateKeyConfig::File { path, kind } => {
                let key = fs::read_to_string(path).context("reading private key file")?;
                let key = hex::decode(key.trim()).context("decoding private key")?;
                match kind {
                    KeyKind::Ed25519 => Ed25519SigningKey::try_from(key.as_ref())?.into(),
                    KeyKind::Secp256k1 => Secp256k1SigningKey::try_from(key.as_ref())?.into(),
                }
            }
        };
        let user_id = UserId::from_bytes(signing_key.public_key().as_bytes());
        let party_id = PartyId::from(user_id.as_ref());
        let mut dependencies = Self::build_dependencies(config.clone(), &signing_key)?;
        let is_leader = config.cluster.leader.public_keys.authentication == signing_key.public_key().as_bytes();
        if is_leader {
            dependencies.leader = Self::build_leader_services(&mut dependencies, &config, &signing_key)?;
        }
        // Export metrics periodically on these repos.
        StorageMetricsExporter::spawn(dependencies.sqlite_repositories.clone());

        info!("Using identity {user_id}");
        let handle = Self::launch_grpc_service(config, party_id, dependencies, preprocessing_mode)?;
        Ok(handle)
    }

    fn build_blob_repository_backend(config: ObjectStorageConfig) -> anyhow::Result<BlobRepositoryBackend> {
        let backend = match config {
            ObjectStorageConfig::AwsS3 { bucket_name, region, endpoint_url, allow_http } => {
                let mut builder = AmazonS3Builder::from_env()
                    .with_client_options(
                        ClientOptions::new()
                            .with_timeout(S3_OPERATION_TIMEOUT)
                            .with_connect_timeout(S3_CONNECT_TIMEOUT),
                    )
                    .with_bucket_name(bucket_name.clone())
                    .with_conditional_put(S3ConditionalPut::ETagMatch);

                if let Some(region) = region {
                    builder = builder.with_region(region);
                }
                if builder.get_config_value(&AmazonS3ConfigKey::Region).is_none() {
                    let region = block_on(resolve_bucket_region(&bucket_name, &ClientOptions::new()))?;
                    builder = builder.with_region(region);
                }
                if let Some(endpoint_url) = endpoint_url {
                    builder = builder.with_endpoint(endpoint_url);
                }
                if let Some(allow_http) = allow_http {
                    builder = builder.with_allow_http(allow_http);
                }

                let client = builder.build()?;
                let object_store = Box::new(client);
                let repo: Box<dyn BlobRepository<u32>> = Box::new(ObjectStoreRepository::new(object_store.clone()));
                block_on(async { repo.check_permissions().await.context("s3 permissions validation") })?;

                BlobRepositoryBackend::S3 { object_store }
            }

            ObjectStorageConfig::InMemory => BlobRepositoryBackend::Memory,
            ObjectStorageConfig::Filesystem { path } => BlobRepositoryBackend::Filesystem(path),
        };
        Ok(backend)
    }

    fn build_dependencies(config: node_config::Config, signing_key: &SigningKey) -> anyhow::Result<Dependencies> {
        let repo_backend = Self::build_blob_repository_backend(config.storage.object_storage)?;
        let program_auditor = ProgramAuditor::new(config.program_auditor.clone());
        let leader_public_key =
            match Self::build_cluster_member(config.cluster.leader.clone())?.public_keys.authentication {
                PublicKey::Ed25519(key) => Ed25519PublicKey::from_bytes(&key)?.into(),
                PublicKey::Secp256k1(key) => Secp256k1PublicKey::from_bytes(&key)?.into(),
            };
        let sqlite = block_on(async { SqliteDb::new(&config.storage.db_url).await })?;
        let nonces_repository = Arc::new(SqliteUsedNoncesRepository::new(sqlite.clone()));
        let blob_expirations_repository = Arc::new(SqliteBlobExpirationsRepository::new(sqlite.clone()));
        let nonces = Arc::new(DefaultNonceService::new(nonces_repository.clone()));
        let sqlite_repositories: Vec<MetricsExporterRepository> =
            vec![nonces_repository.clone(), blob_expirations_repository.clone()];
        let time_service = Arc::new(DefaultTimeService);
        let cluster = Self::build_cluster(config.cluster.clone())?;
        let mut ca_cert = None;
        if let Some(tls) = &config.runtime.grpc.tls {
            if let Some(ca_cert_path) = &tls.ca_cert {
                ca_cert = Some(fs::read(ca_cert_path).context("reading TLS CA certificate file")?);
            }
        }
        let token_dollar_conversion: Arc<dyn TokenDollarConversionService> =
            if let Some(dollar_token_conversion) = config.payments.dollar_token_conversion {
                Arc::new(TokenDollarConversionCoinGeckoService::new(
                    dollar_token_conversion.coingecko_api_key,
                    dollar_token_conversion.coin_id,
                ))
            } else {
                let fixed = Decimal::from_f64(config.payments.dollar_token_conversion_fixed)
                    .ok_or(anyhow!("Invalid fixed token dollar conversion rate: Decimal cannot be from that value"))?;
                warn!("Using fixed token dollar price ({}) because no coingecko configuration was provided", fixed);
                Arc::new(HardcodedTokenDollarConversionService::new(fixed))
            };
        let channels = Arc::new(DefaultClusterChannels::new(signing_key, &cluster, ca_cert)?);
        let dependencies = Dependencies {
            prep_compare: repo_backend.create_preprocessing_service("prep/compare"),
            prep_division_integer_secret: repo_backend.create_preprocessing_service("prep/division_integer_secret"),
            prep_modulo: repo_backend.create_preprocessing_service("prep/modulo"),
            prep_public_output_equality: repo_backend.create_preprocessing_service("prep/public_output_equality"),
            prep_equals_integer_secret: repo_backend.create_preprocessing_service("prep/equals_integer_secret"),
            prep_truncpr: repo_backend.create_preprocessing_service("prep/truncpr"),
            prep_trunc: repo_backend.create_preprocessing_service("prep/trunc"),
            random_integer: repo_backend.create_preprocessing_service("prep/random_integer"),
            random_boolean: repo_backend.create_preprocessing_service("prep/random_boolean"),
            cggmp21_aux_info: repo_backend.create_auxiliary_material_service("aux/cggmp21_aux_info"),
            user_values: Arc::new(DefaultUserValuesService::new(
                Box::new(DefaultBlobService::new("user_values", repo_backend.create_repository())),
                blob_expirations_repository.clone(),
            )),
            results: Arc::new(DefaultResultsService::new(
                Box::new(DefaultBlobService::new("results", repo_backend.create_repository())),
                blob_expirations_repository,
            )),
            time: time_service.clone(),
            programs: Arc::new(DefaultProgramService::new(
                Box::new(DefaultBlobService::new("programs", repo_backend.create_repository())),
                program_auditor.clone(),
            )),
            tx_retriever: Arc::new(DefaultPaymentTransactionRetriever::new(&config.payments.rpc_endpoint)?),
            receipts: Arc::new(DefaultReceiptsService::new(leader_public_key, time_service.clone(), nonces.clone())),
            token_dollar_conversion_service: token_dollar_conversion,
            nonces,
            nonces_repository,
            sqlite,
            leader: None,
            sqlite_repositories,
            compute_api_handles: ComputeApiHandles::default(),
            channels,
            cluster,
            cancel_token: Default::default(),
        };
        Ok(dependencies)
    }

    fn launch_grpc_service(
        config: node_config::Config,
        party_id: PartyId,
        dependencies: Dependencies,
        preprocessing_mode: PreprocessingMode,
    ) -> anyhow::Result<NodeHandle> {
        let mut server_builder = tonic::transport::Server::builder();
        if let Some(tls) = &config.runtime.grpc.tls {
            let cert = fs::read(&tls.cert).context("reading TLS certificate file")?;
            let key = fs::read(&tls.key).context("reading TLS key file")?;
            let identity = Identity::from_pem(cert, key);
            server_builder = server_builder
                .tls_config(ServerTlsConfig::default().identity(identity))
                .context("initializing TLS config")?;
            info!("Starting TLS gRPC server on {}", config.runtime.grpc.bind_endpoint);
        } else {
            info!("Starting insecure gRPC server on {}", config.runtime.grpc.bind_endpoint);
        }
        let leader_user_id = match &dependencies.cluster.leader.public_keys.authentication {
            PublicKey::Ed25519(bytes) => UserId::from_bytes(bytes),
            PublicKey::Secp256k1(bytes) => UserId::from_bytes(bytes),
        };
        let users: Vec<_> = dependencies
            .cluster
            .members
            .iter()
            .map(|m| match &m.public_keys.authentication {
                PublicKey::Ed25519(bytes) => UserId::from_bytes(bytes),
                PublicKey::Secp256k1(bytes) => UserId::from_bytes(bytes),
            })
            .collect();
        let internal_interceptor = InternalServiceInterceptor::new(users.clone());
        let rate_limit_layer = config.runtime.grpc.rate_limit.as_ref().map(|config| {
            let quota = match config.bucket {
                RateLimitBucket::Second => Quota::per_second(config.max_per_bucket),
                RateLimitBucket::Minute => Quota::per_minute(config.max_per_bucket),
                RateLimitBucket::Hour => Quota::per_hour(config.max_per_bucket),
            };
            tonic::service::interceptor(RateLimitInterceptor::new(users, quota))
        });

        let prime_builder = Self::build_prime_builder(party_id.clone(), &dependencies.cluster, config.execution_engine)
            .context("building prime builder")?;
        let auth_interceptor = ServerAuthInterceptor::new(NodeId::from(party_id.as_ref().to_vec()));
        let runtime_elements = Arc::new(DefaultRuntimeElementsService::new(
            dependencies.prep_compare.clone(),
            dependencies.prep_division_integer_secret.clone(),
            dependencies.prep_modulo.clone(),
            dependencies.prep_public_output_equality.clone(),
            dependencies.prep_equals_integer_secret.clone(),
            dependencies.prep_truncpr.clone(),
            dependencies.prep_trunc.clone(),
            dependencies.random_integer.clone(),
            dependencies.random_boolean.clone(),
            dependencies.cggmp21_aux_info.clone(),
        ));

        let max_payload_size = config.network.max_payload_size as usize;

        let preprocessing_api = PreprocessingApi::new(
            party_id.clone(),
            leader_user_id,
            dependencies.channels.clone(),
            prime_builder.clone(),
            PreprocessingApiServices {
                prep_compare: dependencies.prep_compare.clone(),
                prep_division_secret_divisor: dependencies.prep_division_integer_secret.clone(),
                prep_modulo: dependencies.prep_modulo.clone(),
                prep_equality_public_output: dependencies.prep_public_output_equality.clone(),
                prep_equality_secret_output: dependencies.prep_equals_integer_secret.clone(),
                prep_trunc_pr: dependencies.prep_truncpr.clone(),
                prep_trunc: dependencies.prep_trunc.clone(),
                random_integer: dependencies.random_integer.clone(),
                random_boolean: dependencies.random_boolean.clone(),
                cggmp21_aux_info: dependencies.cggmp21_aux_info.clone(),
            },
            preprocessing_mode.clone(),
            dependencies.cancel_token.clone(),
        );
        let preprocessing_server = PreprocessingServer::new(preprocessing_api)
            .max_decoding_message_size(max_payload_size)
            .max_encoding_message_size(max_payload_size);
        let preprocessing_service = InterceptedService::new(preprocessing_server, internal_interceptor);

        let (mut health_reporter, health_service) = health_reporter();
        tokio::spawn(async move {
            health_reporter.set_service_status("", ServingStatus::Serving).await;
        });

        let reflection_service = ReflectionBuilder::configure()
            .register_encoded_file_descriptor_set(TONIC_HEALTH_DESCRIPTOR)
            .build_v1()
            .context("building reflection service")?;

        let mut server = server_builder
            .accept_http1(true)
            .layer(CorsLayer::permissive())
            .layer(GrpcWebLayer::new())
            .layer(tonic::service::interceptor(auth_interceptor))
            .layer(MiddlewareLayer::new(MetricsMiddleware))
            .layer(tower::util::option_layer(rate_limit_layer))
            .add_service(
                MembershipServer::new(MembershipApi::new(dependencies.cluster.clone()))
                    .max_decoding_message_size(max_payload_size)
                    .max_encoding_message_size(max_payload_size),
            )
            .add_service(
                ValuesServer::new(ValuesApi::new(
                    ValuesApiServices {
                        user_values: dependencies.user_values.clone(),
                        receipts: dependencies.receipts.clone(),
                        time: dependencies.time.clone(),
                    },
                    dependencies.cluster.prime.clone(),
                ))
                .max_decoding_message_size(max_payload_size)
                .max_encoding_message_size(max_payload_size),
            )
            .add_service(
                PermissionsServer::new(PermissionsApi::new(PermissionsApiServices {
                    user_values: dependencies.user_values.clone(),
                    receipts: dependencies.receipts.clone(),
                }))
                .max_decoding_message_size(max_payload_size)
                .max_encoding_message_size(max_payload_size),
            )
            .add_service(
                ProgramsServer::new(ProgramsApi::new(ProgramsApiServices {
                    programs: dependencies.programs.clone(),
                    receipts: dependencies.receipts.clone(),
                }))
                .max_decoding_message_size(max_payload_size)
                .max_encoding_message_size(max_payload_size),
            )
            .add_service(
                ComputeServer::new(ComputeApi::new(
                    party_id.clone(),
                    dependencies.channels.clone(),
                    prime_builder.clone(),
                    dependencies.compute_api_handles.clone(),
                    ComputeApiServices {
                        receipts: dependencies.receipts.clone(),
                        programs: dependencies.programs.clone(),
                        user_values: dependencies.user_values.clone(),
                        results: dependencies.results.clone(),
                        runtime_elements,
                    },
                    dependencies.cluster.prime.clone(),
                ))
                .max_decoding_message_size(max_payload_size)
                .max_encoding_message_size(max_payload_size),
            )
            .add_service(preprocessing_service)
            .add_service(health_service)
            .add_service(reflection_service);
        if let Some(leader_dependencies) = dependencies.leader {
            let offsets_service = leader_dependencies.offsets.clone();
            tokio::spawn(async move {
                if let Err(e) = offsets_service.emit_metrics().await {
                    error!("Failed to export initial set of offsets metrics: {e}");
                }
            });
            AuxiliaryMaterialScheduler::spawn(
                dependencies.channels.clone(),
                leader_dependencies.auxiliary_material_metadata_repository.clone(),
                leader_dependencies.auxiliary_material_config.clone(),
                dependencies.cancel_token.clone(),
            );
            server = server
                .add_service(
                    PaymentsServer::new(PaymentsApi::new(
                        dependencies.compute_api_handles.general_compute.clone(),
                        config.runtime.max_concurrent_actions,
                        PaymentsApiServices { payments: leader_dependencies.payments.clone() },
                        Days::new(config.payments.account_balance_expiration_days as u64),
                    ))
                    .max_decoding_message_size(max_payload_size)
                    .max_encoding_message_size(max_payload_size),
                )
                .add_service(
                    LeaderQueriesServer::new(LeaderQueriesApi::new(LeaderQueriesApiServices {
                        receipts: dependencies.receipts.clone(),
                        offsets: leader_dependencies.offsets.clone(),
                        auxiliary_material_metadata: leader_dependencies.auxiliary_material_metadata_service.clone(),
                        preprocessing_config: leader_dependencies.preprocessing_config.clone(),
                    }))
                    .max_decoding_message_size(max_payload_size)
                    .max_encoding_message_size(max_payload_size),
                );
            UsedPreprocessingCleanup::spawn(
                dependencies.channels,
                leader_dependencies.offsets.clone(),
                leader_dependencies.preprocessing_config.clone(),
            );
            BalancesCleanup::spawn(
                leader_dependencies.account_balances.clone(),
                Days::new(config.payments.account_balance_expiration_days as u64),
            );
            if !config.payments.prefunded_accounts.is_empty() {
                block_on(async {
                    Self::prefund_keys(&config.payments.prefunded_accounts, &leader_dependencies).await
                })?;
            }
        }
        NonceCleanup::spawn(dependencies.nonces.clone());
        ExpiredValuesCleanup::spawn(dependencies.user_values.clone());
        ExpiredComputeResultsCleanup::spawn(dependencies.results.clone());

        let (sender, receiver) = oneshot::channel();
        let cancel_token = dependencies.cancel_token.clone();
        let signal = async move {
            if receiver.await.is_err() {
                error!("Signal channel sender dropped");
            }
            info!("Cancelling operations and shutting down");
            cancel_token.cancel();
        };
        let fut = server.serve_with_shutdown(config.runtime.grpc.bind_endpoint, signal);
        let handle = tokio::spawn(async move {
            if let Err(e) = fut.await {
                error!("Failed to serve gRPC server: {e}");
            };
        });
        info!("gRPC server started");
        Ok(NodeHandle { handle, signal: sender })
    }

    async fn prefund_keys(
        prefunded_accounts: &[PrefundedAccount],
        dependencies: &LeaderDependencies,
    ) -> anyhow::Result<()> {
        info!("Pre-funding {} accounts", prefunded_accounts.len());
        let repo = &dependencies.account_balances;
        let mut tx = repo.begin_transaction().await?;
        for account in prefunded_accounts {
            let user: UserId = account.account.parse()?;
            let amount: i64 =
                account.amount.try_into().map_err(|_| anyhow!("prefunded ammount for {user} is too large"))?;
            let current_balance = repo.find(&user, &mut tx).await?.map(|a| a.balance).unwrap_or_default();
            let current_balance = i64::try_from(current_balance)?;
            if current_balance < amount {
                let amount = amount.saturating_sub(current_balance);
                info!("Pre-funding {user} with {amount}");
                repo.add_funds(&user, amount, &mut tx).await?;
            } else {
                info!("Not funding {user} because it already has {current_balance} tokens");
            }
        }
        tx.commit().await?;
        Ok(())
    }

    fn build_cluster(cluster: node_config::Cluster) -> anyhow::Result<node_api::membership::rust::Cluster> {
        let prime = match cluster.prime {
            node_config::Prime::Safe64Bits => Prime::Safe64Bits,
            node_config::Prime::Safe128Bits => Prime::Safe128Bits,
            node_config::Prime::Safe256Bits => Prime::Safe256Bits,
        };

        let cluster = node_api::membership::rust::Cluster {
            members: cluster.members.into_iter().map(Self::build_cluster_member).collect::<Result<_, _>>()?,
            leader: Self::build_cluster_member(cluster.leader)?,
            prime,
            polynomial_degree: cluster.polynomial_degree,
            kappa: cluster.kappa,
        };
        Ok(cluster)
    }

    fn build_cluster_member(member: node_config::ClusterMember) -> anyhow::Result<ClusterMember> {
        let (authentication, identity) = match member.public_keys.kind {
            KeyKind::Ed25519 => {
                let public_key: [u8; 32] = member
                    .public_keys
                    .authentication
                    .try_into()
                    .map_err(|_| anyhow!("ed25519 public key must be 32 bytes long"))?;
                let identity = UserId::from_bytes(public_key).as_ref().to_vec();
                (PublicKey::Ed25519(public_key), identity)
            }
            KeyKind::Secp256k1 => {
                let public_key: [u8; 33] = member
                    .public_keys
                    .authentication
                    .try_into()
                    .map_err(|_| anyhow!("secp256k1 public key must be 33 bytes long"))?;
                let identity = UserId::from_bytes(public_key).as_ref().to_vec();
                (PublicKey::Secp256k1(public_key), identity)
            }
        };
        Ok(ClusterMember {
            identity: NodeId::from(identity),
            public_keys: node_api::membership::rust::PublicKeys { authentication },
            grpc_endpoint: member.grpc_endpoint,
        })
    }

    fn build_prime_builder(
        party_id: PartyId,
        cluster: &node_api::membership::rust::Cluster,
        execution_vm_config: ExecutionVmConfig,
    ) -> anyhow::Result<Arc<dyn PrimeBuilder>> {
        let parties: Vec<_> = cluster.members.iter().map(|m| PartyId::from(Vec::from(m.identity.clone()))).collect();
        let degree = cluster.polynomial_degree as u64;
        let builder: Arc<dyn PrimeBuilder> = match cluster.prime {
            Prime::Safe64Bits => Arc::new(DefaultPrimeBuilder::<U64SafePrime>::new(
                ShamirSecretSharer::new(party_id, degree, parties)?,
                execution_vm_config,
            )),
            Prime::Safe128Bits => Arc::new(DefaultPrimeBuilder::<U128SafePrime>::new(
                ShamirSecretSharer::new(party_id, degree, parties)?,
                execution_vm_config,
            )),
            Prime::Safe256Bits => Arc::new(DefaultPrimeBuilder::<U256SafePrime>::new(
                ShamirSecretSharer::new(party_id, degree, parties)?,
                execution_vm_config,
            )),
        };
        Ok(builder)
    }

    fn build_leader_services(
        dependencies: &mut Dependencies,
        config: &node_config::Config,
        signing_key: &SigningKey,
    ) -> Result<Option<LeaderDependencies>, Error> {
        struct DummyPreprocessingSchedulingService;

        impl PreprocessingSchedulingService for DummyPreprocessingSchedulingService {
            fn notify_used_elements(&self, _elements: &[PreprocessingElement]) {
                error!("Real preprocessing scheduling service not set");
            }
        }

        async fn initialize_offsets_repo(handle: SqliteDb) -> Result<Arc<SqlitePreprocessingOffsetsRepository>, Error> {
            let repo = SqlitePreprocessingOffsetsRepository::new(handle);
            for element in PreprocessingElement::iter() {
                repo.register_element(element).await?;
            }
            Ok(Arc::new(repo))
        }

        let Some(preprocessing_config) = config.network.preprocessing.clone() else {
            return Ok(None);
        };
        let PaymentsConfig { pricing, quote_ttl, receipt_ttl, minimum_add_funds_payment, .. } = config.payments.clone();
        let payments_service_config = PaymentsServiceConfig {
            max_payload_size: config.network.max_payload_size,
            pricing,
            preprocessing: preprocessing_config.clone(),
            quote_ttl,
            receipt_ttl,
            minimum_add_funds_credits: minimum_add_funds_payment.into(),
        };
        let balances_repository = Arc::new(SqliteAccountBalanceRepository::new(dependencies.sqlite.clone()));
        let transfers_repository = Arc::new(SqliteTransfersRepository::new(dependencies.sqlite.clone()));
        let offsets_repository = block_on(async { initialize_offsets_repo(dependencies.sqlite.clone()).await })?;
        let offsets = Arc::new(DefaultElementOffsetsService::new(
            offsets_repository.clone(),
            // Use a dummy one temporarily because we have a cyclic dependency here
            Arc::new(DummyPreprocessingSchedulingService),
        ));
        let handle = PreprocessingScheduler::spawn(
            dependencies.channels.clone(),
            preprocessing_config.clone(),
            PreprocessingSchedulerServices { offsets: offsets.clone(), uuid: Arc::new(DefaultUuidService) },
            dependencies.cancel_token.clone(),
        );
        let preprocessing_scheduling = Arc::new(DefaultPreprocessingSchedulingService::new(handle));
        offsets.set_preprocessing_scheduliing_service(preprocessing_scheduling.clone());

        let auxiliary_material_metadata_repository =
            Arc::new(SqliteAuxiliaryMaterialMetadataRepository::new(dependencies.sqlite.clone()));
        let auxiliary_material_metadata =
            Arc::new(DefaultAuxiliaryMaterialMetadataService::new(auxiliary_material_metadata_repository.clone()));
        let payments = Arc::new(DefaultPaymentService::new(
            signing_key.clone(),
            PaymentServiceDependencies {
                time_service: dependencies.time.clone(),
                programs_service: dependencies.programs.clone(),
                balance_repo: balances_repository.clone(),
                transfers_repo: transfers_repository.clone(),
                used_nonces_repo: dependencies.nonces_repository.clone(),
                tx_retriever: dependencies.tx_retriever.clone(),
                offsets_service: offsets.clone(),
                auxiliary_material_metadata_service: auxiliary_material_metadata.clone(),
                token_dollar_conversion_service: dependencies.token_dollar_conversion_service.clone(),
            },
            payments_service_config,
        )?);
        dependencies.sqlite_repositories.push(offsets_repository.clone());
        dependencies.sqlite_repositories.push(balances_repository.clone());
        dependencies.sqlite_repositories.push(transfers_repository);
        dependencies.sqlite_repositories.push(auxiliary_material_metadata_repository.clone());

        Ok(Some(LeaderDependencies {
            payments,
            offsets,
            auxiliary_material_metadata_repository,
            auxiliary_material_metadata_service: auxiliary_material_metadata,
            preprocessing_config,
            auxiliary_material_config: config.network.auxiliary_material.clone().unwrap_or_default(),
            account_balances: balances_repository,
        }))
    }

    /// Initialize the prometheus metrics exporter.
    pub async fn initialize_metrics(config: &MetricsConfig) -> Result<(), Error> {
        let hostname = hostname::get()?.to_string_lossy().to_string();
        let mut labels = HashMap::from([("hostname".to_string(), hostname)]);
        labels.extend(config.static_labels.clone().into_iter());
        let exporter =
            PrometheusExporter::new(labels).map_err(|e| anyhow!("failed to create prometheus exporter: {e}"))?;
        let process_metrics_collector = ProcessMetricsCollector::default();
        let interval = config.process_collector_interval;
        tokio::spawn(async move { process_metrics_collector.run(interval).await });
        exporter.launch(config.listen_address);
        Ok(())
    }
}

enum BlobRepositoryBackend {
    Memory,
    Filesystem(PathBuf),
    S3 { object_store: Box<AmazonS3> },
}

impl BlobRepositoryBackend {
    fn create_repository<T>(&self) -> Box<dyn BlobRepository<T>>
    where
        T: BinarySerde + Clone,
    {
        use BlobRepositoryBackend::*;
        match &self {
            Memory => Box::new(MemoryBlobRepository::default()),
            Filesystem(path) => Box::new(FilesystemBlobRepository::new(path.clone())),
            S3 { object_store: client } => Box::new(ObjectStoreRepository::new(client.clone())),
        }
    }

    fn create_preprocessing_service<T>(&self, prefix: &str) -> Arc<dyn PreprocessingBlobService<T>>
    where
        T: BinarySerde + Serialize + DeserializeOwned + Clone,
    {
        let repo = self.create_repository();
        Arc::new(DefaultPreprocessingBlobService::new(Box::new(DefaultBlobService::new(prefix, repo))))
    }

    fn create_auxiliary_material_service<T>(&self, prefix: &str) -> Arc<dyn AuxiliaryMaterialService<T>>
    where
        T: BinarySerde + Clone,
    {
        let repo = self.create_repository();
        Arc::new(DefaultAuxiliaryMaterialService::new(Box::new(DefaultBlobService::new(prefix, repo))))
    }
}

/// A handle a running instance of a node.
pub struct NodeHandle {
    handle: JoinHandle<()>,
    signal: oneshot::Sender<()>,
}

impl NodeHandle {
    /// Shutdown this node gracefully.
    pub async fn shutdown(self) {
        info!("Sending the shutdown signal");
        if self.signal.send(()).is_err() {
            error!("Shutdown signal receiver dropped");
            return;
        }
        match timeout(GRACEFUL_SHUTDOWN_TIMEOUT, self.handle).await {
            Ok(Ok(_)) => info!("Node has shutdown"),
            Ok(Err(_)) => info!("Node has failed to shutdown"),
            Err(_) => info!("Timed out waiting for node to shutdown"),
        }
    }
}
