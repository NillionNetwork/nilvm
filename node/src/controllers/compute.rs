//! Compute gRPC API.

use super::{extract_values, InvalidReceiptType};
use crate::{
    channels::ClusterChannels,
    controllers::{compute::proto::stream::ComputeType, TraceRequest},
    services::{
        programs::ProgramService,
        receipts::ReceiptsService,
        results::{FetchResultError, OutputPartyResult, ResultsService},
        runtime_elements::{PreprocessingElementOffsets, PreprocessingElementsPlan, RuntimeElementsService},
        user_values::{UserValuesAccessReason, UserValuesService},
    },
    stateful::{
        builder::{BuildExecutionVmError, ExecutionVm, PrimeBuilder},
        compute::{ExecutionVmIo, StateMetadata},
        distributed_key_generation::{create_user_outputs, EcdsaDistributedKeyGenerationIo},
        sm::{
            InitMessage, StandardStateMachine, StateMachine, StateMachineArgs, StateMachineHandle, StateMachineRunner,
        },
        SIGNAL_CHANNEL_SIZE, STREAM_CHANNEL_SIZE,
    },
};
use async_trait::async_trait;
use basic_types::PartyId;
use encoding::codec::MessageCodec;
use futures::StreamExt;
use grpc_channel::auth::AuthenticateRequest;
use math_lib::modular::EncodedModulo;
use mpc_vm::{
    protocols::MPCProtocol,
    vm::{plan::MPCRuntimePreprocessingElements, MPCExecutionVmMessage},
    Program,
};
use nada_compiler_backend::program_contract::ProgramContract;
use nada_value::{
    encrypted::{Encoded, Encrypted},
    protobuf::nada_values_from_protobuf,
    NadaValue,
};
use node_api::{
    auth::rust::UserId,
    compute::{
        proto,
        rust::{
            InputPartyBinding, InvokeComputeRequest, InvokeComputeResponse, OutputPartyBinding, RetrieveResultsRequest,
            RetrieveResultsResponse,
        },
        TECDSA_DKG_PROGRAM_ID,
    },
    membership::rust::Prime,
    payments::rust::{
        InvokeCompute, InvokeComputeMetadata, OperationMetadata, Receipt, SelectedAuxiliaryMaterial,
        SelectedPreprocessingOffsets,
    },
    values::rust::NamedValue,
    ConvertProto, TryIntoRust,
};
use protocols::distributed_key_generation::dkg::{EcdsaKeyGenOutput, EcdsaKeyGenState, EcdsaKeyGenStateMessage};
use std::{
    collections::{hash_map::Entry, HashMap, HashSet},
    marker::PhantomData,
    sync::Arc,
    time::Duration,
};
use tokio::{
    sync::{
        mpsc::{channel, error::SendError, Receiver, Sender},
        Mutex,
    },
    time::timeout,
};
use tokio_stream::wrappers::ReceiverStream;
use tonic::{Request, Response, Status, Streaming};
use tracing::{error, info, instrument, warn};
use uuid::Uuid;

// The timeout for each request sent to another node during a compute flow.
const COMPUTE_REQUEST_TIMEOUT: Duration = Duration::from_secs(30);

const WAIT_COMPUTE_NOTIFY_INTERVAL: Duration = Duration::from_secs(30);

const STATE_MACHINE_NAME: &str = "COMPUTE";

type RetrieveResultsStream = ReceiverStream<tonic::Result<proto::retrieve::RetrieveResultsResponse>>;

pub(crate) type ComputeHandles = Arc<Mutex<HashMap<Uuid, StateMachineHandle<ExecutionVmIo>>>>;
pub(crate) type EcdsaDkgComputeHandles = Arc<Mutex<HashMap<Uuid, StateMachineHandle<EcdsaDistributedKeyGenerationIo>>>>;
pub(crate) struct ComputeApiServices {
    pub(crate) receipts: Arc<dyn ReceiptsService>,
    pub(crate) programs: Arc<dyn ProgramService>,
    pub(crate) user_values: Arc<dyn UserValuesService>,
    pub(crate) results: Arc<dyn ResultsService>,
    pub(crate) runtime_elements: Arc<dyn RuntimeElementsService>,
}

pub(crate) struct ComputeApi {
    our_party_id: PartyId,
    channels: Arc<dyn ClusterChannels>,
    prime_builder: Arc<dyn PrimeBuilder>,
    compute_handles: ComputeHandles,
    ecdsa_dkg_compute_handles: EcdsaDkgComputeHandles,
    services: ComputeApiServices,
    modulo: EncodedModulo,
}

impl ComputeApi {
    pub(crate) fn new(
        our_party_id: PartyId,
        channels: Arc<dyn ClusterChannels>,
        prime_builder: Arc<dyn PrimeBuilder>,
        compute_handles: ComputeHandles,
        ecdsa_dkg_compute_handles: EcdsaDkgComputeHandles,
        services: ComputeApiServices,
        prime: Prime,
    ) -> Self {
        let modulo = match prime {
            Prime::Safe64Bits => EncodedModulo::U64SafePrime,
            Prime::Safe128Bits => EncodedModulo::U128SafePrime,
            Prime::Safe256Bits => EncodedModulo::U256SafePrime,
        };
        Self { our_party_id, channels, prime_builder, compute_handles, ecdsa_dkg_compute_handles, services, modulo }
    }

    fn transform_stream<T>(peer: UserId, mut stream: Streaming<proto::stream::ComputeStreamMessage>) -> Receiver<T>
    where
        T: serde::de::DeserializeOwned + Send + 'static,
    {
        let (tx, rx) = channel(STREAM_CHANNEL_SIZE);
        // Used to costomize the stream type in the logs below
        let stream_type =
            if std::any::type_name::<T>().contains("EcdsaKeyGenStateMessage") { "DKG" } else { "Compute" };

        tokio::spawn(async move {
            while let Some(msg) = stream.next().await {
                match msg {
                    Ok(msg) => match MessageCodec.decode::<T>(&msg.bincode_message) {
                        Ok(msg) => {
                            match tx.send(msg).await {
                                Ok(_) => continue,
                                Err(_) => {
                                    warn!(
                                        "{stream_type} stream receiver was dropped while there were still messages queued up"
                                    );
                                }
                            };
                        }
                        Err(e) => {
                            warn!("Failed to decode {stream_type} message from {peer}: {e}");
                        }
                    },
                    Err(e) => {
                        warn!("Received error from peer {peer}: {e}");
                    }
                };
                break;
            }
        });
        rx
    }

    #[allow(clippy::too_many_arguments)]
    async fn build_vm(
        &self,
        compute_id: Uuid,
        user_id: &UserId,
        program_id: String,
        program: Program<MPCProtocol>,
        value_ids: &[Vec<u8>],
        values: Vec<NamedValue>,
        selected_offsets: &[SelectedPreprocessingOffsets],
        selected_auxiliary_materials: &[SelectedAuxiliaryMaterial],
    ) -> tonic::Result<ExecutionVm> {
        let values = self.build_values(user_id, program_id, value_ids, values).await?;
        let runtime_elements =
            self.fetch_runtime_elements(selected_offsets, selected_auxiliary_materials).await.map_err(|e| {
                error!("Failed to fetch runtime elements: {e}");
                Status::internal("failed to fetch runtime elements")
            })?;
        let vm =
            self.prime_builder.build_execution_vm(program, values, runtime_elements, compute_id).inspect_err(|e| {
                if let BuildExecutionVmError::CreatingVm(_) = e {
                    error!("Failed to create execution VM: {e}");
                }
            })?;
        Ok(vm)
    }

    async fn build_values(
        &self,
        user_id: &UserId,
        program_id: String,
        value_ids: &[Vec<u8>],
        values: Vec<NamedValue>,
    ) -> tonic::Result<HashMap<String, NadaValue<Encrypted<Encoded>>>> {
        let value_ids = value_ids
            .iter()
            .map(|id| Uuid::from_slice(id))
            .collect::<Result<Vec<_>, _>>()
            .map_err(|_| Status::invalid_argument("values_id is not a uuid"))?;
        let raw_values = self
            .services
            .user_values
            .find_many(&value_ids, user_id, &UserValuesAccessReason::Compute { program_id })
            .await?;
        let mut values = nada_values_from_protobuf(values, &self.modulo)
            .map_err(|e| Status::invalid_argument(format!("invalid values: {e}")))?;
        for user_value in raw_values {
            let user_value = nada_values_from_protobuf(user_value.values, &self.modulo).map_err(|e| {
                error!("Found corrupted set of values ({value_ids:?}): {e}");
                Status::internal("corrupted values found")
            })?;
            for (key, value) in user_value {
                match values.entry(key) {
                    Entry::Occupied(entry) => {
                        return Err(Status::invalid_argument(format!("duplicate value key '{}'", entry.key())));
                    }
                    Entry::Vacant(entry) => {
                        entry.insert(value);
                    }
                };
            }
        }
        Ok(values)
    }

    async fn fetch_runtime_elements(
        &self,
        selected_offsets: &[SelectedPreprocessingOffsets],
        selected_auxiliary_materials: &[SelectedAuxiliaryMaterial],
    ) -> anyhow::Result<MPCRuntimePreprocessingElements> {
        let mut preprocessing_plan = PreprocessingElementsPlan::default();
        for selected_offsets in selected_offsets {
            let offset_range = selected_offsets.start..selected_offsets.end;
            let offsets = PreprocessingElementOffsets::from_range(offset_range, selected_offsets.batch_size);
            preprocessing_plan.0.insert(selected_offsets.element, offsets);
        }
        Ok(self.services.runtime_elements.request_elements(preprocessing_plan, selected_auxiliary_materials).await?)
    }

    fn validate_input_bindings(bindings: &[InputPartyBinding], contract: &ProgramContract) -> tonic::Result<()> {
        let parties = contract.input_parties().map_err(|e| Status::internal(format!("malformed program: {e}")))?;
        let bindings = bindings.iter().map(|b| &b.party_name);
        Self::validate_bindings(bindings, parties, "input")
    }

    fn validate_output_bindings(bindings: &[OutputPartyBinding], contract: &ProgramContract) -> tonic::Result<()> {
        let parties = contract.output_parties().map_err(|e| Status::internal(format!("malformed program: {e}")))?;
        let bindings = bindings.iter().map(|b| &b.party_name);
        Self::validate_bindings(bindings, parties, "output")
    }

    fn validate_bindings<'a, I1, I2>(bindings: I1, required_parties: I2, party_type: &str) -> tonic::Result<()>
    where
        I1: IntoIterator<Item = &'a String>,
        I2: IntoIterator<Item = &'a nada_compiler_backend::mir::Party>,
    {
        let mut required_parties: HashSet<_> = required_parties.into_iter().map(|p| &p.name).collect();
        for name in bindings {
            if !required_parties.remove(name) {
                return Err(Status::invalid_argument(format!("{party_type} party {name} is not defined in program")))?;
            }
        }
        // if we have any left, it means some aren't bound
        match required_parties.len() {
            0 => Ok(()),
            n if n <= 3 => {
                Err(Status::invalid_argument(format!("{party_type} parties {required_parties:?} not bound")))
            }
            n => Err(Status::invalid_argument(format!("{n} {party_type} parties not bound"))),
        }
    }

    fn build_metadata(
        output_bindings: Vec<OutputPartyBinding>,
        program: &Program<MPCProtocol>,
    ) -> anyhow::Result<StateMetadata> {
        let outputs = program.contract.outputs_by_party_name()?;
        StateMetadata::new(output_bindings, outputs)
    }

    fn compute_args(&self, compute_id: Uuid) -> StateMachineArgs<ExecutionVmIo> {
        let io = ExecutionVmIo { compute_id, results_service: self.services.results.clone() };
        StateMachineArgs {
            id: compute_id,
            our_party_id: self.our_party_id.clone(),
            channels: self.channels.clone(),
            timeout: COMPUTE_REQUEST_TIMEOUT,
            name: STATE_MACHINE_NAME,
            io,
            handles: self.compute_handles.clone(),
            // We don't want to cancel active compute operations
            cancel_token: Default::default(),
        }
    }

    fn ecdsa_dkg_compute_args(&self, compute_id: Uuid) -> StateMachineArgs<EcdsaDistributedKeyGenerationIo> {
        let io = EcdsaDistributedKeyGenerationIo {
            compute_id,
            results_service: self.services.results.clone(),
            user_values_service: self.services.user_values.clone(),
            _unused: PhantomData,
        };
        StateMachineArgs {
            id: compute_id,
            our_party_id: self.our_party_id.clone(),
            channels: self.channels.clone(),
            timeout: COMPUTE_REQUEST_TIMEOUT,
            name: "ECDSA_DKG",
            io,
            handles: self.ecdsa_dkg_compute_handles.clone(),
            // We don't want to cancel active compute operations
            cancel_token: Default::default(),
        }
    }

    async fn wait_execution_result(
        results_service: Arc<dyn ResultsService>,
        compute_id: Uuid,
        user_id: UserId,
        sender: Sender<tonic::Result<proto::retrieve::RetrieveResultsResponse>>,
    ) -> Result<(), SendError<tonic::Result<proto::retrieve::RetrieveResultsResponse>>> {
        sender.send(Ok(RetrieveResultsResponse::WaitingComputation.into_proto())).await?;
        loop {
            // Attempt to wait for some amount of time
            let result =
                match timeout(WAIT_COMPUTE_NOTIFY_INTERVAL, results_service.wait_execution(compute_id, &user_id)).await
                {
                    Ok(Ok(result)) => result,
                    Ok(Err(e)) => {
                        sender.send(Err(e.into())).await?;
                        return Ok(());
                    }
                    Err(_) => {
                        // On timeout, send a message and keep looping
                        sender.send(Ok(RetrieveResultsResponse::WaitingComputation.into_proto())).await?;
                        continue;
                    }
                };
            // If we have the result, great, otherwise fetch it
            let result = match result {
                Some(result) => Ok(result),
                None => results_service.fetch_output_party_result(compute_id, &user_id).await,
            };
            let result = match result {
                Ok(r) => r,
                Err(e) => {
                    sender.send(Err(e.into())).await?;
                    return Ok(());
                }
            };
            let response = match result {
                OutputPartyResult::Success { values } => RetrieveResultsResponse::Success { values },
                OutputPartyResult::Failure { error } => RetrieveResultsResponse::Error { error },
            };
            sender.send(Ok(response.into_proto())).await?;
            break;
        }
        Ok(())
    }

    async fn handle_ecdsa_dkg_compute(
        &self,
        identifier: Vec<u8>,
        output_bindings: Vec<OutputPartyBinding>,
    ) -> tonic::Result<Response<proto::invoke::InvokeComputeResponse>> {
        let compute_id = Uuid::from_slice(&identifier).map_err(|_| Status::internal("invalid uuid"))?;
        let user_outputs = create_user_outputs(&output_bindings)?;

        // Create a new state machine for DKG
        let eid = compute_id.to_string().as_bytes().to_vec();
        let parties = self.channels.all_parties().iter().map(|p| p.party_id.clone()).collect();
        let (state, initial_messages) = EcdsaKeyGenState::new(eid, parties, self.our_party_id.clone())
            .map_err(|e| Status::internal(format!("Failed to create DKG state machine: {e}")))?;
        let sm = state_machine::StateMachine::new(state);
        let sm = StandardStateMachine::<EcdsaKeyGenState, EcdsaKeyGenStateMessage>::new(sm, initial_messages);
        let vm: Box<
            dyn StateMachine<Message = EcdsaKeyGenStateMessage, Result = anyhow::Result<Vec<EcdsaKeyGenOutput>>>,
        > = Box::new(sm);

        // Register execution in results service
        self.services.results.register_execution(compute_id).await;

        // Insert state machine handle
        let mut handles = self.ecdsa_dkg_compute_handles.lock().await;
        handles
            .entry(compute_id)
            .or_insert_with(|| {
                info!("Initializing DKG compute id {compute_id}");
                let args = self.ecdsa_dkg_compute_args(compute_id);
                StateMachineRunner::start(args)
            })
            .send(InitMessage::InitStateMachine { state_machine: vm, metadata: StateMetadata { user_outputs } })
            .await
            .map_err(|e| {
                error!("State machine handle dropped: {e}");
                Status::internal("internal error")
            })?;
        drop(handles);

        let response = InvokeComputeResponse { compute_id: compute_id.into() }.into_proto();
        Ok(Response::new(response))
    }

    #[allow(clippy::too_many_arguments)]
    async fn handle_general_compute(
        &self,
        user_id: UserId,
        identifier: Vec<u8>,
        quote: InvokeCompute,
        value_ids: Vec<Vec<u8>>,
        values: Vec<NamedValue>,
        input_bindings: Vec<InputPartyBinding>,
        output_bindings: Vec<OutputPartyBinding>,
        offsets: Vec<SelectedPreprocessingOffsets>,
        auxiliary_materials: Vec<SelectedAuxiliaryMaterial>,
    ) -> tonic::Result<Response<proto::invoke::InvokeComputeResponse>> {
        let values = extract_values(values, &self.modulo)?;
        let program_id = quote.program_id.parse()?;
        let program = self.services.programs.find(&program_id).await?;
        Self::validate_input_bindings(&input_bindings, &program.contract)?;
        Self::validate_output_bindings(&output_bindings, &program.contract)?;

        let metadata = Self::build_metadata(output_bindings, &program)
            .map_err(|e| Status::invalid_argument(format!("invalid program: {e}")))?;
        let compute_id = Uuid::from_slice(&identifier).map_err(|_| Status::internal("invalid uuid"))?;
        let state_machine = self
            .build_vm(
                compute_id,
                &user_id,
                quote.program_id,
                program,
                &value_ids,
                values,
                &offsets,
                &auxiliary_materials,
            )
            .await?;
        self.services.results.register_execution(compute_id).await;

        // Insert an entry for this compute id if it doesn't exist yet
        let mut handles = self.compute_handles.lock().await;
        handles
            .entry(compute_id)
            .or_insert_with(|| {
                info!("Initializing compute id {compute_id}");
                let args = self.compute_args(compute_id);
                StateMachineRunner::start(args)
            })
            .send(InitMessage::InitStateMachine { state_machine, metadata })
            .await
            .map_err(|e| {
                error!("State machine handle dropped: {e}");
                Status::internal("internal error")
            })?;
        drop(handles);

        let response = InvokeComputeResponse { compute_id: compute_id.into() }.into_proto();
        Ok(Response::new(response))
    }
}

impl From<BuildExecutionVmError> for Status {
    fn from(e: BuildExecutionVmError) -> Self {
        use BuildExecutionVmError::*;
        match e {
            CreatingVm(_) => {
                error!("Failed to create VM: {e}");
                Status::internal("internal error")
            }
            InvalidValues(_) | InputValidation(_) => Status::invalid_argument(e.to_string()),
        }
    }
}

#[async_trait]
impl proto::compute_server::Compute for ComputeApi {
    type RetrieveResultsStream = RetrieveResultsStream;

    #[instrument(name = "api.compute.invoke_compute", skip_all, fields(user_id = request.trace_user_id()))]
    async fn invoke_compute(
        &self,
        request: Request<proto::invoke::InvokeComputeRequest>,
    ) -> tonic::Result<Response<proto::invoke::InvokeComputeResponse>> {
        let user_id = request.user_id()?;
        let InvokeComputeRequest { signed_receipt, value_ids, values, input_bindings, output_bindings } =
            request.into_inner().try_into_rust()?;
        let Receipt { identifier, metadata, .. } =
            self.services.receipts.verify_payment_receipt(signed_receipt).await?;
        let OperationMetadata::InvokeCompute(InvokeComputeMetadata { quote, offsets, auxiliary_materials }) = metadata
        else {
            return Err(InvalidReceiptType("invoke compute").into());
        };

        // Check for special DKG program
        if quote.program_id == TECDSA_DKG_PROGRAM_ID {
            return self.handle_ecdsa_dkg_compute(identifier, output_bindings.clone()).await;
        }
        // Handle general compute case
        self.handle_general_compute(
            user_id,
            identifier,
            quote,
            value_ids,
            values,
            input_bindings,
            output_bindings,
            offsets,
            auxiliary_materials,
        )
        .await
    }

    #[instrument(name = "api.compute.stream_compute", skip_all)]
    async fn stream_compute(
        &self,
        stream: Request<Streaming<proto::stream::ComputeStreamMessage>>,
    ) -> tonic::Result<Response<()>> {
        let user_id = stream.user_id()?;
        if !self.channels.is_member(&user_id) {
            return Err(Status::permission_denied("not a member of this cluster"));
        }
        let mut stream = stream.into_inner();
        let msg = match stream.next().await {
            Some(Ok(msg)) => msg,
            Some(Err(_)) => return Err(Status::invalid_argument("error receiving compute stream header")),
            None => return Err(Status::invalid_argument("no compute stream header provided")),
        };
        if msg.compute_id.is_empty() {
            return Err(Status::invalid_argument("expected init message as first stream message"));
        };
        let compute_id = Uuid::from_slice(&msg.compute_id).map_err(|_| Status::internal("invalid uuid"))?;

        match msg.compute_type() {
            ComputeType::EcdsaDkg => {
                let stream = Self::transform_stream::<EcdsaKeyGenStateMessage>(user_id, stream);
                // Insert an entry for this compute if it's not present. We could receive this call before
                // the client gets to talk to us so this can happen under normal circumstances.
                let mut handles = self.ecdsa_dkg_compute_handles.lock().await;
                handles
                    .entry(compute_id)
                    .or_insert_with(|| {
                        info!("Initializing compute id {compute_id}");
                        let args = self.ecdsa_dkg_compute_args(compute_id);
                        StateMachineRunner::start(args)
                    })
                    .send(InitMessage::InitParty { user_id, stream })
                    .await
                    .map_err(|e| {
                        error!("Dkg state machine handle dropped: {e}");
                        Status::internal("internal error")
                    })?;
            }
            ComputeType::General => {
                let stream = Self::transform_stream::<MPCExecutionVmMessage>(user_id, stream);
                // Insert an entry for this compute if it's not present. We could receive this call before
                // the client gets to talk to us so this can happen under normal circumstances.
                let mut handles = self.compute_handles.lock().await;
                handles
                    .entry(compute_id)
                    .or_insert_with(|| {
                        info!("Initializing compute id {compute_id}");
                        let args = self.compute_args(compute_id);
                        StateMachineRunner::start(args)
                    })
                    .send(InitMessage::InitParty { user_id, stream })
                    .await
                    .map_err(|e| {
                        error!("State machine handle dropped: {e}");
                        Status::internal("internal error")
                    })?;
            }
        }
        Ok(Response::new(()))
    }

    #[instrument(name = "api.compute.retrieve_results", skip_all, fields(user_id = request.trace_user_id()))]
    async fn retrieve_results(
        &self,
        request: Request<proto::retrieve::RetrieveResultsRequest>,
    ) -> tonic::Result<Response<Self::RetrieveResultsStream>> {
        let user_id = request.user_id()?;
        let request: RetrieveResultsRequest = request.into_inner().try_into_rust()?;
        let compute_id = Uuid::from_slice(&request.compute_id).map_err(|_| Status::internal("invalid uuid"))?;
        let (tx, rx) = channel(SIGNAL_CHANNEL_SIZE);

        info!("Retrieving compute results with id {compute_id}");
        let results_service = self.services.results.clone();
        tokio::spawn(async move {
            if Self::wait_execution_result(results_service, compute_id, user_id, tx.clone()).await.is_err() {
                warn!("Client disconnected before we could send compute result");
            }
        });
        Ok(Response::new(ReceiverStream::new(rx)))
    }
}

impl From<FetchResultError> for Status {
    fn from(e: FetchResultError) -> Self {
        use FetchResultError::*;
        match e {
            Unauthorized => Status::permission_denied(e.to_string()),
            Blob(e) => e.into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        channels::MockClusterChannels,
        services::{
            programs::MockProgramService, receipts::MockReceiptsService, results::MockResultsService,
            runtime_elements::MockRuntimeElementsService, user_values::MockUserValuesService,
        },
        stateful::{
            builder::MockPrimeBuilder,
            sm::{EncodedYield, StateMachine},
        },
        storage::models::user_values::UserValuesRecord,
    };
    use basic_types::PartyMessage;
    use chrono::Utc;
    use math_lib::modular::{EncodedModularNumber, EncodedModulo};
    use mockall::predicate::{always, eq};
    use nada_value::protobuf::nada_values_to_protobuf;
    use node_api::{
        membership::rust::Prime,
        permissions::rust::Permissions,
        preprocessing::rust::{AuxiliaryMaterial, PreprocessingElement},
    };
    use rstest::rstest;
    use std::ops::Range;
    use test_programs::PROGRAMS;

    #[derive(Default)]
    struct MockExecutionVm;

    impl StateMachine for MockExecutionVm {
        type Result = HashMap<String, NadaValue<Encrypted<Encoded>>>;
        type Message = MPCExecutionVmMessage;

        fn initialize(&mut self) -> anyhow::Result<EncodedYield<Self::Result, Self::Message>> {
            Ok(EncodedYield::Empty)
        }

        fn proceed(
            &mut self,
            _: PartyMessage<Self::Message>,
        ) -> anyhow::Result<EncodedYield<Self::Result, Self::Message>> {
            Ok(EncodedYield::Empty)
        }
    }

    #[derive(Default)]
    struct ServiceBuilder {
        programs: MockProgramService,
        receipts: MockReceiptsService,
        user_values: MockUserValuesService,
        results: MockResultsService,
        prime_builder: MockPrimeBuilder,
        channels: MockClusterChannels,
        runtime_elements: MockRuntimeElementsService,
    }

    impl ServiceBuilder {
        fn build(self) -> ComputeApi {
            let our_party_id = PartyId::from(vec![]);
            let prime_builder = Arc::new(self.prime_builder);
            let channels = Arc::new(self.channels);

            ComputeApi::new(
                our_party_id,
                channels,
                prime_builder,
                Default::default(),
                Default::default(),
                ComputeApiServices {
                    programs: Arc::new(self.programs),
                    receipts: Arc::new(self.receipts),
                    user_values: Arc::new(self.user_values),
                    results: Arc::new(self.results),
                    runtime_elements: Arc::new(self.runtime_elements),
                },
                Prime::Safe64Bits,
            )
        }
    }

    fn make_selected_offsets(
        element: node_api::preprocessing::rust::PreprocessingElement,
        range: Range<u64>,
    ) -> SelectedPreprocessingOffsets {
        SelectedPreprocessingOffsets { element, start: range.start, end: range.end, batch_size: 1 }
    }

    fn make_offsets(range: Range<u32>) -> PreprocessingElementOffsets {
        // These numbers go in line with `make_selected_offsets` above
        PreprocessingElementOffsets {
            first_batch_id: range.start,
            last_batch_id: range.start,
            start_offset: 0,
            total: 1,
        }
    }

    #[tokio::test]
    async fn create_vm() {
        let mut builder = ServiceBuilder::default();
        let program_id = "test".to_string();
        let user_id = UserId::from_bytes("bob");
        // our inputs
        let input_values: HashMap<String, NadaValue<Encrypted<Encoded>>> = HashMap::from([(
            "A".to_string(),
            NadaValue::new_integer(EncodedModularNumber::new_unchecked(vec![1], EncodedModulo::U64SafePrime)),
        )]);
        // the repo's inputs
        let repo_values: HashMap<String, NadaValue<Encrypted<Encoded>>> = HashMap::from([(
            "B".to_string(),
            NadaValue::new_integer(EncodedModularNumber::new_unchecked(vec![2], EncodedModulo::U64SafePrime)),
        )]);
        // what we expect to see: all inputs joined together
        let all_values: HashMap<_, _> = input_values.clone().into_iter().chain(repo_values.clone()).collect();
        let protobuf_values = nada_values_to_protobuf(input_values).unwrap();
        let program = PROGRAMS.program("simple_shares").unwrap().0;
        builder.programs.expect_requirements().return_once(|_| Ok(Default::default()));
        builder.runtime_elements.expect_request_elements().return_once(|_, _| Ok(Default::default()));
        builder.user_values.expect_find_many().return_once(|_, _, _| {
            Ok(vec![UserValuesRecord {
                values: nada_values_to_protobuf(repo_values).unwrap(),
                permissions: Permissions {
                    owner: UserId::from_bytes("bob"),
                    retrieve: Default::default(),
                    update: Default::default(),
                    delete: Default::default(),
                    compute: Default::default(),
                },
                expires_at: Utc::now(),
                prime: Prime::Safe64Bits,
            }])
        });
        builder
            .prime_builder
            .expect_build_execution_vm()
            .with(always(), eq(all_values.clone()), always(), always())
            .return_once(|_, _, _, _| Ok(Box::new(MockExecutionVm::default())));

        let service = builder.build();
        service.build_vm(Uuid::new_v4(), &user_id, program_id, program, &[], protobuf_values, &[], &[]).await.unwrap();
    }

    #[tokio::test]
    async fn create_vm_preprocessing_offsets() {
        let mut builder = ServiceBuilder::default();
        let program_id = "test".to_string();
        let user_id = UserId::from_bytes("bob");
        // our inputs
        let input_values: HashMap<String, NadaValue<Encrypted<Encoded>>> = HashMap::new();
        let protobuf_values = nada_values_to_protobuf(input_values).unwrap();

        use node_api::preprocessing::rust::PreprocessingElement as E;
        let selected_offsets = &[
            make_selected_offsets(E::Compare, 1..2),
            make_selected_offsets(E::DivisionSecretDivisor, 2..3),
            make_selected_offsets(E::EqualitySecretOutput, 3..4),
            make_selected_offsets(E::EqualityPublicOutput, 4..5),
            make_selected_offsets(E::Modulo, 6..7),
            make_selected_offsets(E::Trunc, 7..8),
            make_selected_offsets(E::TruncPr, 8..9),
        ];
        let selected_auxiliary_materials =
            vec![SelectedAuxiliaryMaterial { material: AuxiliaryMaterial::Cggmp21AuxiliaryInfo, version: 42 }];
        let plan = PreprocessingElementsPlan(
            [
                (PreprocessingElement::Compare, make_offsets(1..2)),
                (PreprocessingElement::DivisionSecretDivisor, make_offsets(2..3)),
                (PreprocessingElement::EqualitySecretOutput, make_offsets(3..4)),
                (PreprocessingElement::EqualityPublicOutput, make_offsets(4..5)),
                (PreprocessingElement::Modulo, make_offsets(6..7)),
                (PreprocessingElement::Trunc, make_offsets(7..8)),
                (PreprocessingElement::TruncPr, make_offsets(8..9)),
            ]
            .into(),
        );

        let program = PROGRAMS.program("simple_shares").unwrap().0;
        builder.user_values.expect_find_many().return_once(|_, _, _| Ok(Vec::new()));
        builder
            .runtime_elements
            .expect_request_elements()
            .with(eq(plan), eq(selected_auxiliary_materials.clone()))
            .return_once(|_, _| Ok(Default::default()));
        builder
            .prime_builder
            .expect_build_execution_vm()
            .with(always(), eq(HashMap::new()), always(), always())
            .return_once(|_, _, _, _| Ok(Box::new(MockExecutionVm::default())));

        let service = builder.build();
        service
            .build_vm(
                Uuid::new_v4(),
                &user_id,
                program_id,
                program,
                &[],
                protobuf_values,
                selected_offsets,
                &selected_auxiliary_materials,
            )
            .await
            .unwrap();
    }

    #[test]
    fn binding_validation_success() {
        let bindings = &["a".to_string()];
        let parties =
            &[nada_compiler_backend::mir::Party { name: "a".to_string(), source_ref_index: Default::default() }];
        ComputeApi::validate_bindings(bindings, parties.iter(), "test").expect("validation failed");
    }

    #[rstest]
    #[case::too_many_parties(&["a", "b"], &["a"])]
    #[case::too_few_parties(&["a"], &["a", "b"])]
    #[case::no_overlap(&["a"], &["b"])]
    fn binding_validation_failure(#[case] bindings: &[&str], #[case] required: &[&str]) {
        let bindings: Vec<_> = bindings.into_iter().map(|s| s.to_string()).collect();
        let parties: Vec<_> = required
            .into_iter()
            .map(|n| nada_compiler_backend::mir::Party { name: n.to_string(), source_ref_index: Default::default() })
            .collect();
        ComputeApi::validate_bindings(bindings.iter(), parties.iter(), "test").expect_err("not an error");
    }
}
