use crate::{
    builder::PreprocessingMode,
    channels::ClusterChannels,
    controllers::TraceRequest,
    services::{
        auxiliary_material::AuxiliaryMaterialService,
        preprocessing::{FakePreprocessingBlobService, PreprocessingBlobService},
    },
    stateful::{
        auxiliary_material::{AuxiliaryMaterialMetadata, AuxiliaryMaterialStateMachineIo},
        builder::PrimeBuilder,
        preprocessing::{FakePreprocessingStateMachine, PreprocessingMetadata, PreprocessingStateMachineIo},
        sm::{
            BoxStateMachine, InitMessage, StandardStateMachineState, StateMachine, StateMachineArgs,
            StateMachineHandle, StateMachineMessage, StateMachineRunner,
        },
    },
    storage::repositories::blob::BlobRepositoryError,
};
use anyhow::{anyhow, bail};
use async_trait::async_trait;
use basic_types::PartyId;
use futures::StreamExt;
use grpc_channel::auth::AuthenticateRequest;
use math_lib::modular::{EncodedModularNumber, U64SafePrime};
use node_api::{
    auth::rust::UserId,
    preprocessing::{
        proto::{self},
        rust::{
            AuxiliaryMaterial, AuxiliaryMaterialStreamMessage, CleanupUsedElementsRequest,
            GenerateAuxiliaryMaterialRequest, GenerateAuxiliaryMaterialResponse, GeneratePreprocessingRequest,
            PreprocessingElement, PreprocessingProtocolStatus, PreprocessingStreamMessage,
        },
    },
    ConvertProto, TryIntoRust,
};
use protocols::{
    conditionals::{
        equality::offline::{EncodedPrepPrivateOutputEqualityShares, PrepPrivateOutputEqualityState},
        equality_public_output::offline::{
            state::PrepPublicOutputEqualityState, EncodedPrepPublicOutputEqualityShares,
        },
        less_than::offline::{state::PrepCompareState, EncodedPrepCompareShares},
    },
    division::{
        division_secret_divisor::offline::{
            state::PrepDivisionIntegerSecretState, EncodedPrepDivisionIntegerSecretShares,
        },
        modulo2m_public_divisor::offline::{state::PrepModulo2mState, EncodedPrepModulo2mShares},
        modulo_public_divisor::offline::{state::PrepModuloState, EncodedPrepModuloShares},
        truncation_probabilistic::offline::{state::PrepTruncPrState, EncodedPrepTruncPrShares},
    },
    random::{
        random_bit::{EncodedBitShare, RandomBitState},
        random_integer::RandomIntegerState,
    },
    threshold_ecdsa::auxiliary_information::{
        fake::FakeEcdsaAuxInfo,
        output::{EcdsaAuxInfo, EcdsaAuxInfoOutput},
        EcdsaAuxInfoState,
    },
};
use std::{
    collections::HashMap,
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::{
    sync::{
        mpsc::{channel, Receiver, Sender},
        Mutex,
    },
    task::spawn_blocking,
};
use tokio_stream::wrappers::ReceiverStream;
use tokio_util::sync::CancellationToken;
use tonic::{Request, Response, Status, Streaming};
use tracing::{error, info, instrument};
use uuid::Uuid;

type DummyPrime = U64SafePrime;

// The timeout for each request sent to another node during a preprocessing flow.
const PREPROCESSING_REQUEST_TIMEOUT: Duration = Duration::from_secs(15);

// The timeout for each request sent to another node during an auxiliary material generation flow.
const AUXILIARY_MATERIAL_REQUEST_TIMEOUT: Duration = Duration::from_secs(120);

const SMALL_CHANNEL_SIZE: usize = 16;
const LARGE_CHANNEL_SIZE: usize = 1024;

pub(crate) type PreprocessingHandles<I> = Arc<Mutex<HashMap<Uuid, StateMachineHandle<PreprocessingStateMachineIo<I>>>>>;
pub(crate) type AuxiliaryMaterialHandles<I> =
    Arc<Mutex<HashMap<Uuid, StateMachineHandle<AuxiliaryMaterialStateMachineIo<I>>>>>;

#[derive(Default)]
struct Handles {
    // All of this use `U64SafePrime` (via `DummyPrime`) as a placeholder. The messages used by
    // `PrepCompareState<T>` are the same for every `T` so it doesn't matter here.
    prep_compare: PreprocessingHandles<PrepCompareState<DummyPrime>>,
    prep_division_secret_divisor: PreprocessingHandles<PrepDivisionIntegerSecretState<DummyPrime>>,
    prep_modulo: PreprocessingHandles<PrepModuloState<DummyPrime>>,
    prep_equality_public_output: PreprocessingHandles<PrepPublicOutputEqualityState<DummyPrime>>,
    prep_equality_secret_output: PreprocessingHandles<PrepPrivateOutputEqualityState<DummyPrime>>,
    prep_trunc_pr: PreprocessingHandles<PrepTruncPrState<DummyPrime>>,
    prep_trunc: PreprocessingHandles<PrepModulo2mState<DummyPrime>>,
    random_integer: PreprocessingHandles<RandomIntegerState<DummyPrime>>,
    random_boolean: PreprocessingHandles<RandomBitState<DummyPrime>>,
    cggmp21_aux_info: AuxiliaryMaterialHandles<EcdsaAuxInfoState>,
}

// Fake preprocessing services.
//
// Fake preprocessing is used during tests and when running a local devnet, given we don't want to
// burn CPU unnecessarily. The way it works is:
//
// * The first time preprocessing runs, the batch size is altered to be 1 internally, so that we do
// run the preprocessing protocol, but only to generate a single share.
// * When inserting the share into the repository, we copy/paste it `batch_size` times.
// * Any subsequent preprocessing will use a dummy state machine that states it's done immediately,
// and upon storing the shares in the repository it will take that first share, replicate it
// `batch_size` times.
//
// This requires a few hacks to work. Specifically:
//
// * We use fake blob services that implement the copy/paste behavior described above (e.g. they
// save the first share in memory and replicate it every time).
// * We create fake state machines that return immediately.
struct FakeServices {
    prep_compare: Arc<FakePreprocessingBlobService<EncodedPrepCompareShares>>,
    prep_division_secret_divisor: Arc<FakePreprocessingBlobService<EncodedPrepDivisionIntegerSecretShares>>,
    prep_modulo: Arc<FakePreprocessingBlobService<EncodedPrepModuloShares>>,
    prep_equality_public_output: Arc<FakePreprocessingBlobService<EncodedPrepPublicOutputEqualityShares>>,
    prep_equality_secret_output: Arc<FakePreprocessingBlobService<EncodedPrepPrivateOutputEqualityShares>>,
    prep_trunc_pr: Arc<FakePreprocessingBlobService<EncodedPrepTruncPrShares>>,
    prep_trunc: Arc<FakePreprocessingBlobService<EncodedPrepModulo2mShares>>,
    random_integer: Arc<FakePreprocessingBlobService<EncodedModularNumber>>,
    random_boolean: Arc<FakePreprocessingBlobService<EncodedBitShare>>,
}

pub(crate) struct PreprocessingApiServices {
    pub(crate) prep_compare: Arc<dyn PreprocessingBlobService<EncodedPrepCompareShares>>,
    pub(crate) prep_division_secret_divisor: Arc<dyn PreprocessingBlobService<EncodedPrepDivisionIntegerSecretShares>>,
    pub(crate) prep_modulo: Arc<dyn PreprocessingBlobService<EncodedPrepModuloShares>>,
    pub(crate) prep_equality_public_output: Arc<dyn PreprocessingBlobService<EncodedPrepPublicOutputEqualityShares>>,
    pub(crate) prep_equality_secret_output: Arc<dyn PreprocessingBlobService<EncodedPrepPrivateOutputEqualityShares>>,
    pub(crate) prep_trunc_pr: Arc<dyn PreprocessingBlobService<EncodedPrepTruncPrShares>>,
    pub(crate) prep_trunc: Arc<dyn PreprocessingBlobService<EncodedPrepModulo2mShares>>,
    pub(crate) random_integer: Arc<dyn PreprocessingBlobService<EncodedModularNumber>>,
    pub(crate) random_boolean: Arc<dyn PreprocessingBlobService<EncodedBitShare>>,
    pub(crate) cggmp21_aux_info: Arc<dyn AuxiliaryMaterialService<EcdsaAuxInfo>>,
}

pub(crate) struct PreprocessingApi {
    our_party_id: PartyId,
    leader_user: UserId,
    prime_builder: Arc<dyn PrimeBuilder>,
    channels: Arc<dyn ClusterChannels>,
    services: PreprocessingApiServices,
    handles: Handles,
    fake_services: Option<FakeServices>,
    cancel_token: CancellationToken,
}

impl PreprocessingApi {
    pub(crate) fn new(
        our_party_id: PartyId,
        leader_user: UserId,
        channels: Arc<dyn ClusterChannels>,
        prime_builder: Arc<dyn PrimeBuilder>,
        services: PreprocessingApiServices,
        mode: PreprocessingMode,
        cancel_token: CancellationToken,
    ) -> Self {
        let fake_services = match mode {
            PreprocessingMode::Real => None,
            PreprocessingMode::Fake => Some(FakeServices {
                prep_compare: FakePreprocessingBlobService::new(services.prep_compare.clone()).into(),
                prep_division_secret_divisor: FakePreprocessingBlobService::new(
                    services.prep_division_secret_divisor.clone(),
                )
                .into(),
                prep_modulo: FakePreprocessingBlobService::new(services.prep_modulo.clone()).into(),
                prep_equality_public_output: FakePreprocessingBlobService::new(
                    services.prep_equality_public_output.clone(),
                )
                .into(),
                prep_equality_secret_output: FakePreprocessingBlobService::new(
                    services.prep_equality_secret_output.clone(),
                )
                .into(),
                prep_trunc_pr: FakePreprocessingBlobService::new(services.prep_trunc_pr.clone()).into(),
                prep_trunc: FakePreprocessingBlobService::new(services.prep_trunc.clone()).into(),
                random_integer: FakePreprocessingBlobService::new(services.random_integer.clone()).into(),
                random_boolean: FakePreprocessingBlobService::new(services.random_boolean.clone()).into(),
            }),
        };
        Self {
            our_party_id,
            leader_user,
            prime_builder,
            channels,
            services,
            fake_services,
            handles: Default::default(),
            cancel_token,
        }
    }

    fn preprocessing_args<S>(
        &self,
        name: &'static str,
        io: PreprocessingStateMachineIo<S>,
        handles: PreprocessingHandles<S>,
    ) -> StateMachineArgs<PreprocessingStateMachineIo<S>>
    where
        S: StandardStateMachineState<proto::stream::PreprocessingStreamMessage>,
    {
        let id = io.generation_id;
        StateMachineArgs {
            id,
            our_party_id: self.our_party_id.clone(),
            channels: self.channels.clone(),
            timeout: PREPROCESSING_REQUEST_TIMEOUT,
            name,
            io,
            handles,
            cancel_token: Some(self.cancel_token.clone()),
        }
    }

    fn auxiliary_material_args<S>(
        &self,
        name: &'static str,
        io: AuxiliaryMaterialStateMachineIo<S>,
        handles: AuxiliaryMaterialHandles<S>,
    ) -> StateMachineArgs<AuxiliaryMaterialStateMachineIo<S>>
    where
        S: StandardStateMachineState<proto::stream::AuxiliaryMaterialStreamMessage>,
    {
        let id = io.generation_id;
        StateMachineArgs {
            id,
            our_party_id: self.our_party_id.clone(),
            channels: self.channels.clone(),
            timeout: AUXILIARY_MATERIAL_REQUEST_TIMEOUT,
            name,
            io,
            handles,
            cancel_token: Some(self.cancel_token.clone()),
        }
    }

    async fn initialize_preprocessing_protocol_generation<S>(
        &self,
        batch_id: u64,
        handles: &PreprocessingHandles<S>,
        state_machine: BoxStateMachine<PreprocessingStateMachineIo<S>>,
        io: PreprocessingStateMachineIo<S>,
        response_channel: Sender<tonic::Result<proto::generate::GeneratePreprocessingResponse>>,
        name: &'static str,
    ) -> anyhow::Result<()>
    where
        S: StandardStateMachineState<proto::stream::PreprocessingStreamMessage>,
    {
        let generation_id = io.generation_id;
        let args = self.preprocessing_args(name, io, handles.clone());
        let metadata = PreprocessingMetadata { response_channel, batch_id };
        handles
            .lock()
            .await
            .entry(generation_id)
            .or_insert_with(|| {
                info!("Initializing generation id {generation_id} for {name}");
                StateMachineRunner::start(args)
            })
            .send(InitMessage::InitStateMachine { state_machine, metadata })
            .await
            .map_err(|_| anyhow!("preprocessing generation shutdown"))
    }

    async fn initialize_auxiliary_material_protocol_generation<S>(
        &self,
        version: u32,
        handles: &AuxiliaryMaterialHandles<S>,
        state_machine: BoxStateMachine<AuxiliaryMaterialStateMachineIo<S>>,
        io: AuxiliaryMaterialStateMachineIo<S>,
        response_channel: Sender<tonic::Result<proto::generate::GenerateAuxiliaryMaterialResponse>>,
        name: &'static str,
    ) -> anyhow::Result<()>
    where
        S: StandardStateMachineState<proto::stream::AuxiliaryMaterialStreamMessage>,
    {
        let generation_id = io.generation_id;
        let args = self.auxiliary_material_args(name, io, handles.clone());
        let metadata = AuxiliaryMaterialMetadata { response_channel, version };
        handles
            .lock()
            .await
            .entry(generation_id)
            .or_insert_with(|| {
                info!("Initializing generation id {generation_id} for {name}");
                StateMachineRunner::start(args)
            })
            .send(InitMessage::InitStateMachine { state_machine, metadata })
            .await
            .map_err(|_| anyhow!("auxiliary material generation shutdown"))
    }

    async fn initialize_preprocessing_protocol_peer_stream<S>(
        &self,
        user_id: UserId,
        handles: &PreprocessingHandles<S>,
        io: PreprocessingStateMachineIo<S>,
        mut stream: Streaming<proto::stream::PreprocessingStreamMessage>,
        name: &'static str,
    ) -> anyhow::Result<()>
    where
        S: StandardStateMachineState<proto::stream::PreprocessingStreamMessage>,
    {
        let (sender, receiver) = channel(LARGE_CHANNEL_SIZE);
        tokio::spawn(async move {
            while let Some(Ok(msg)) = stream.next().await {
                match S::OutputMessage::try_decode(&msg.bincode_message) {
                    Ok(decoded) => {
                        let _ = sender.send(decoded).await;
                    }
                    Err(e) => {
                        error!("Received invalid message, dropping channel: {e}");
                    }
                };
            }
        });
        let generation_id = io.generation_id;
        let args = self.preprocessing_args(name, io, handles.clone());
        handles
            .lock()
            .await
            .entry(generation_id)
            .or_insert_with(|| {
                info!("Initializing generation id {generation_id} for {name}");
                StateMachineRunner::start(args)
            })
            .send(InitMessage::InitParty { user_id, stream: receiver })
            .await
            .map_err(|_| anyhow!("preprocessing generation shutdown"))
    }

    async fn initialize_auxiliary_material_protocol_peer_stream<S>(
        &self,
        user_id: UserId,
        handles: &AuxiliaryMaterialHandles<S>,
        io: AuxiliaryMaterialStateMachineIo<S>,
        mut stream: Streaming<proto::stream::AuxiliaryMaterialStreamMessage>,
        name: &'static str,
    ) -> anyhow::Result<()>
    where
        S: StandardStateMachineState<proto::stream::AuxiliaryMaterialStreamMessage>,
    {
        let (sender, receiver) = channel(LARGE_CHANNEL_SIZE);
        tokio::spawn(async move {
            while let Some(Ok(msg)) = stream.next().await {
                match S::OutputMessage::try_decode(&msg.bincode_message) {
                    Ok(decoded) => {
                        let _ = sender.send(decoded).await;
                    }
                    Err(e) => {
                        error!("Received invalid message, dropping channel: {e}");
                    }
                };
            }
        });
        let generation_id = io.generation_id;
        let args = self.auxiliary_material_args(name, io, handles.clone());
        handles
            .lock()
            .await
            .entry(generation_id)
            .or_insert_with(|| {
                info!("Initializing generation id {generation_id} for {name}");
                StateMachineRunner::start(args)
            })
            .send(InitMessage::InitParty { user_id, stream: receiver })
            .await
            .map_err(|_| anyhow!("auxiliary material generation shutdown"))
    }

    async fn initialize_preprocessing_generation(
        &self,
        generation_id: Uuid,
        batch_id: u64,
        batch_size: usize,
        element: PreprocessingElement,
    ) -> anyhow::Result<Receiver<tonic::Result<proto::generate::GeneratePreprocessingResponse>>> {
        macro_rules! impl_dispatch {
            ($field:ident) => {{
                paste::paste!{
                    let (sender, receiver) = channel(SMALL_CHANNEL_SIZE);
                    match &self.fake_services {
                        Some(services) => {
                            // Set the batch size and start a fake preprocessing instance.
                            services.$field.set_batch_size(batch_size).await;

                            let state_machine: Box<dyn StateMachine<Message = _, Result = _>> = match batch_id {
                                // For the first batch initialize a real state machine that will
                                // generate 1 share
                                0 => self.prime_builder.[<build_ $field _state_machine>](1)?,
                                // For any other batch generate a fake state machine that will end
                                // immediately
                                _ => Box::new(FakePreprocessingStateMachine::default()),
                            };
                            // let state_machine = self.prime_builder.[<build_ $field _state_machine>](batch_size)?;
                            let io = PreprocessingStateMachineIo::new(generation_id, services.$field.clone(), element);
                            self.initialize_preprocessing_protocol_generation(batch_id, &self.handles.$field, state_machine, io, sender, stringify!{[<$field:upper>]})
                                .await?;
                        },
                        None => {
                            let state_machine = self.prime_builder.[<build_ $field _state_machine>](batch_size)?;
                            let io = PreprocessingStateMachineIo::new(generation_id, self.services.$field.clone(), element);
                            self.initialize_preprocessing_protocol_generation(batch_id, &self.handles.$field, state_machine, io, sender, stringify!{[<$field:upper>]})
                                .await?;
                        }
                    };
                    receiver
                }
            }}
        }

        let now = Instant::now();
        let receiver = match element {
            PreprocessingElement::Compare => {
                impl_dispatch!(prep_compare)
            }
            PreprocessingElement::DivisionSecretDivisor => {
                impl_dispatch!(prep_division_secret_divisor)
            }
            PreprocessingElement::EqualitySecretOutput => {
                impl_dispatch!(prep_equality_secret_output)
            }
            PreprocessingElement::EqualityPublicOutput => {
                impl_dispatch!(prep_equality_public_output)
            }
            PreprocessingElement::Modulo => {
                impl_dispatch!(prep_modulo)
            }
            PreprocessingElement::Trunc => {
                impl_dispatch!(prep_trunc)
            }
            PreprocessingElement::TruncPr => {
                impl_dispatch!(prep_trunc_pr)
            }
            PreprocessingElement::RandomInteger => {
                impl_dispatch!(random_integer)
            }
            PreprocessingElement::RandomBoolean => {
                impl_dispatch!(random_boolean)
            }
        };
        info!("{element} initialization took {:?}", now.elapsed());
        Ok(receiver)
    }

    async fn initialize_auxiliary_material_generation(
        &self,
        generation_id: Uuid,
        version: u32,
        material: AuxiliaryMaterial,
    ) -> anyhow::Result<Receiver<tonic::Result<proto::generate::GenerateAuxiliaryMaterialResponse>>> {
        let now = Instant::now();
        let (sender, receiver) = channel(SMALL_CHANNEL_SIZE);
        if self.fake_services.is_some() {
            match material {
                AuxiliaryMaterial::Cggmp21AuxiliaryInfo => {
                    let info = FakeEcdsaAuxInfo::generate_ecdsa(self.channels.all_parties().len() as u16)?;
                    match info {
                        EcdsaAuxInfoOutput::Success { element } => {
                            self.services.cggmp21_aux_info.upsert(version, element).await?;
                        }
                        EcdsaAuxInfoOutput::Abort { reason } => {
                            bail!("failed to create shares: {reason}")
                        }
                    }
                }
            };
            sender
                .send(Ok(GenerateAuxiliaryMaterialResponse { status: PreprocessingProtocolStatus::FinishedSuccess }
                    .into_proto()))
                .await?;
        } else {
            let execution_id = generation_id.as_bytes().to_vec();

            match material {
                AuxiliaryMaterial::Cggmp21AuxiliaryInfo => {
                    let prime_builder = self.prime_builder.clone();
                    // We spawn this in a separate blocking runtime because this can take several
                    // seconds and that can causes the tokio runtime to halt
                    let state_machine =
                        spawn_blocking(move || prime_builder.build_cggmp21_aux_info_state_machine(execution_id))
                            .await
                            .map_err(|_| anyhow!("failed to initialize state machine"))??;
                    let io = AuxiliaryMaterialStateMachineIo::new(
                        generation_id,
                        self.services.cggmp21_aux_info.clone(),
                        material,
                    );
                    self.initialize_auxiliary_material_protocol_generation(
                        version,
                        &self.handles.cggmp21_aux_info,
                        state_machine,
                        io,
                        sender,
                        "CGGMP21_AUX_INFO",
                    )
                    .await?;
                }
            };
        }
        info!("{material} initialization took {:?}", now.elapsed());
        Ok(receiver)
    }

    async fn initialize_preprocessing_peer_stream(
        &self,
        user_id: UserId,
        generation_id: Uuid,
        element: PreprocessingElement,
        stream: Streaming<proto::stream::PreprocessingStreamMessage>,
    ) -> anyhow::Result<()> {
        macro_rules! impl_dispatch {
            ($field:ident) => {{
                paste::paste! {
                    match &self.fake_services {
                        Some(services) => {
                            let io = PreprocessingStateMachineIo::new(generation_id, services.$field.clone(), element);
                            self.initialize_preprocessing_protocol_peer_stream(user_id, &self.handles.$field, io, stream, stringify!([<$field:upper>])).await
                        }
                        None => {
                            let io = PreprocessingStateMachineIo::new(generation_id, self.services.$field.clone(), element);
                            self.initialize_preprocessing_protocol_peer_stream(user_id, &self.handles.$field, io, stream, stringify!([<$field:upper>])).await
                        }
                    }
                }
            }};
        }
        match element {
            PreprocessingElement::Compare => {
                impl_dispatch!(prep_compare)
            }
            PreprocessingElement::DivisionSecretDivisor => {
                impl_dispatch!(prep_division_secret_divisor)
            }
            PreprocessingElement::EqualitySecretOutput => {
                impl_dispatch!(prep_equality_secret_output)
            }
            PreprocessingElement::EqualityPublicOutput => {
                impl_dispatch!(prep_equality_public_output)
            }
            PreprocessingElement::Modulo => {
                impl_dispatch!(prep_modulo)
            }
            PreprocessingElement::Trunc => {
                impl_dispatch!(prep_trunc)
            }
            PreprocessingElement::TruncPr => {
                impl_dispatch!(prep_trunc_pr)
            }
            PreprocessingElement::RandomInteger => {
                impl_dispatch!(random_integer)
            }
            PreprocessingElement::RandomBoolean => {
                impl_dispatch!(random_boolean)
            }
        }
    }

    async fn initialize_auxiliary_material_peer_stream(
        &self,
        user_id: UserId,
        generation_id: Uuid,
        material: AuxiliaryMaterial,
        stream: Streaming<proto::stream::AuxiliaryMaterialStreamMessage>,
    ) -> tonic::Result<()> {
        if self.fake_services.is_some() {
            return Err(Status::failed_precondition(
                "auxiliary material generation peer streams are not allowed in fake preprocessing mode",
            ));
        }
        match material {
            AuxiliaryMaterial::Cggmp21AuxiliaryInfo => {
                let io = AuxiliaryMaterialStateMachineIo::new(
                    generation_id,
                    self.services.cggmp21_aux_info.clone(),
                    material,
                );
                self.initialize_auxiliary_material_protocol_peer_stream(
                    user_id,
                    &self.handles.cggmp21_aux_info,
                    io,
                    stream,
                    "CGGMP21_AUX_INFO",
                )
                .await
                .map_err(|e| {
                    error!("Failed to initialize auxiliary material generation: {e}");
                    Status::internal("failed to start auxiliary material generation")
                })
            }
        }
    }
}

#[async_trait]
impl proto::preprocessing_server::Preprocessing for PreprocessingApi {
    type GeneratePreprocessingStream = ReceiverStream<tonic::Result<proto::generate::GeneratePreprocessingResponse>>;
    type GenerateAuxiliaryMaterialStream =
        ReceiverStream<tonic::Result<proto::generate::GenerateAuxiliaryMaterialResponse>>;

    #[instrument(name = "api.preprocessing.generate_preprocessing", skip_all, fields(user_id = request.trace_user_id()))]
    async fn generate_preprocessing(
        &self,
        request: Request<proto::generate::GeneratePreprocessingRequest>,
    ) -> tonic::Result<Response<Self::GeneratePreprocessingStream>> {
        let user = request.user_id()?;
        if user != self.leader_user {
            return Err(Status::permission_denied("only leader can invoke this endpoint"));
        }
        let GeneratePreprocessingRequest { generation_id, batch_id, batch_size, element } =
            request.into_inner().try_into_rust()?;
        let generation_id = Uuid::from_slice(&generation_id).map_err(|_| Status::internal("invalid uuid"))?;

        info!(
            "Received initialize preprocessing generation message from {user} for element {element:?}, generation {generation_id}, batch id {batch_id}, batch size {batch_size}"
        );
        match self.initialize_preprocessing_generation(generation_id, batch_id, batch_size as usize, element).await {
            Ok(receiver) => Ok(Response::new(ReceiverStream::new(receiver))),
            Err(e) => {
                error!("Failed to initialize preprocessing: {e}");
                Err(Status::internal("failed to start preprocessing"))
            }
        }
    }

    #[instrument(name = "api.preprocessing.generate_auxiliary_material", skip_all, fields(user_id = request.trace_user_id()))]
    async fn generate_auxiliary_material(
        &self,
        request: Request<proto::generate::GenerateAuxiliaryMaterialRequest>,
    ) -> tonic::Result<Response<Self::GenerateAuxiliaryMaterialStream>> {
        let user = request.user_id()?;
        if user != self.leader_user {
            return Err(Status::permission_denied("only leader can invoke this endpoint"));
        }
        let GenerateAuxiliaryMaterialRequest { generation_id, material, version } =
            request.into_inner().try_into_rust()?;
        let generation_id = Uuid::from_slice(&generation_id).map_err(|_| Status::internal("invalid uuid"))?;

        info!(
            "Received initialize auxiliary material generation message from {user} for material {material:?}, generation {generation_id}, version {version}"
        );
        match self.initialize_auxiliary_material_generation(generation_id, version, material).await {
            Ok(receiver) => Ok(Response::new(ReceiverStream::new(receiver))),
            Err(e) => {
                error!("Failed to initialize auxiliary material generation: {e}");
                Err(Status::internal("failed to start auxiliary material generation"))
            }
        }
    }

    #[instrument(name = "api.preprocessing.stream_preprocessing", skip_all, fields(user_id = stream.trace_user_id()))]
    async fn stream_preprocessing(
        &self,
        stream: Request<Streaming<proto::stream::PreprocessingStreamMessage>>,
    ) -> tonic::Result<Response<()>> {
        let user = stream.user_id()?;
        let mut stream = stream.into_inner();
        let msg: PreprocessingStreamMessage = match stream.next().await {
            Some(Ok(msg)) => msg.try_into_rust()?,
            Some(Err(_)) => return Err(Status::invalid_argument("error receiving preprocessing stream header")),
            None => return Err(Status::invalid_argument("no preprocessing stream header provided")),
        };
        let generation_id = Uuid::from_slice(&msg.generation_id).map_err(|_| Status::internal("invalid uuid"))?;
        match self.initialize_preprocessing_peer_stream(user, generation_id, msg.element, stream).await {
            Ok(()) => Ok(Response::new(())),
            Err(e) => {
                error!("Failed to initialize preprocessing: {e}");
                Err(Status::internal("failed to start preprocessing"))
            }
        }
    }

    #[instrument(name = "api.preprocessing.stream_auxiliary_material", skip_all, fields(user_id = stream.trace_user_id()))]
    async fn stream_auxiliary_material(
        &self,
        stream: Request<Streaming<proto::stream::AuxiliaryMaterialStreamMessage>>,
    ) -> tonic::Result<Response<()>> {
        let user = stream.user_id()?;
        let mut stream = stream.into_inner();
        let msg: AuxiliaryMaterialStreamMessage = match stream.next().await {
            Some(Ok(msg)) => msg.try_into_rust()?,
            Some(Err(_)) => return Err(Status::invalid_argument("error receiving auxiliary material stream header")),
            None => return Err(Status::invalid_argument("no auxiliary material stream header provided")),
        };
        let generation_id = Uuid::from_slice(&msg.generation_id).map_err(|_| Status::internal("invalid uuid"))?;
        self.initialize_auxiliary_material_peer_stream(user, generation_id, msg.material, stream)
            .await
            .map(Response::new)
    }

    #[instrument(name = "api.preprocessing.cleanup_used_elements", skip_all, fields(user_id = request.trace_user_id()))]
    async fn cleanup_used_elements(
        &self,
        request: Request<proto::cleanup::CleanupUsedElementsRequest>,
    ) -> tonic::Result<Response<()>> {
        let user = request.user_id()?;
        if user != self.leader_user {
            return Err(Status::permission_denied("only leader can invoke this endpoint"));
        }
        let request: CleanupUsedElementsRequest = request.into_inner().try_into_rust()?;
        let chunks = request.start_chunk..request.end_chunk;
        let element = request.element;
        info!("Need to delete chunks {chunks:?} for element {element:?}");
        for chunk in chunks {
            let chunk = chunk as u32;
            let fut = match element {
                PreprocessingElement::Compare => self.services.prep_compare.delete(chunk),
                PreprocessingElement::DivisionSecretDivisor => self.services.prep_division_secret_divisor.delete(chunk),
                PreprocessingElement::EqualitySecretOutput => self.services.prep_equality_secret_output.delete(chunk),
                PreprocessingElement::EqualityPublicOutput => self.services.prep_equality_public_output.delete(chunk),
                PreprocessingElement::Modulo => self.services.prep_modulo.delete(chunk),
                PreprocessingElement::Trunc => self.services.prep_trunc.delete(chunk),
                PreprocessingElement::TruncPr => self.services.prep_trunc_pr.delete(chunk),
                PreprocessingElement::RandomInteger => self.services.random_integer.delete(chunk),
                PreprocessingElement::RandomBoolean => self.services.random_boolean.delete(chunk),
            };
            info!("Deleting chunk {chunk} for element {element:?}");
            match fut.await {
                Ok(_) => {
                    info!("Deleted chunk {chunk} for element {element:?}");
                }
                Err(e) => {
                    if let Some(BlobRepositoryError::NotFound) = e.downcast_ref::<BlobRepositoryError>() {
                        info!("Chunk {chunk} for element {element:?} not found, ignoring delete request");
                    } else {
                        error!("Failed to delete chunk {chunk} for element {element:?}: {e}");
                        return Err(Status::internal(format!("failed to delete chunk {chunk}")));
                    }
                }
            };
        }
        Ok(Response::new(()))
    }
}
