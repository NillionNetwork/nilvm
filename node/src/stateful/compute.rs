use super::sm::{EncodedYield, StateMachine, StateMachineIo, StateMachineMessage};
use crate::{channels::ClusterChannels, services::results::ResultsService, storage::models::result::ComputeResult};
use anyhow::{bail, Context};
use async_trait::async_trait;
use basic_types::{PartyId, PartyMessage};
use encoding::codec::MessageCodec;
use math_lib::modular::SafePrime;
use mpc_vm::{
    protocols::MPCProtocol,
    vm::{MPCExecutionVmMessage, VmYield},
};
use nada_compiler_backend::program_contract::Output;
use nada_value::{
    encoders::EncodableWithP,
    encrypted::{Encoded, Encrypted},
    protobuf::nada_values_to_protobuf,
    NadaValue,
};
use node_api::{
    auth::rust::UserId,
    compute::{
        proto::stream::ComputeType,
        rust::{ComputeStreamMessage, OutputPartyBinding},
    },
    values::rust::NamedValue,
};
use shamir_sharing::secret_sharer::{SafePrimeSecretSharer, ShamirSecretSharer};
use std::{collections::HashMap, sync::Arc};
use tokio::sync::mpsc::Sender;
use tracing::error;
use uuid::Uuid;

#[derive(Clone, Debug)]
pub(crate) struct UserOutputs {
    pub(crate) user: UserId,
    pub(crate) outputs: Vec<String>,
}

impl<T> StateMachine for mpc_vm::vm::ExecutionVm<MPCProtocol, T>
where
    T: SafePrime,
    ShamirSecretSharer<T>: SafePrimeSecretSharer<T>,
{
    type Result = HashMap<String, NadaValue<Encrypted<Encoded>>>;
    type Message = MPCExecutionVmMessage;

    fn initialize(&mut self) -> anyhow::Result<EncodedYield<Self::Result, Self::Message>> {
        self.initialize()?.try_into()
    }

    fn proceed(
        &mut self,
        message: PartyMessage<Self::Message>,
    ) -> anyhow::Result<EncodedYield<Self::Result, Self::Message>> {
        self.proceed(message)?.try_into()
    }
}

impl<T> TryFrom<VmYield<MPCProtocol, T>>
    for EncodedYield<HashMap<String, NadaValue<Encrypted<Encoded>>>, MPCExecutionVmMessage>
where
    T: SafePrime,
    ShamirSecretSharer<T>: SafePrimeSecretSharer<T>,
{
    type Error = anyhow::Error;

    fn try_from(value: VmYield<MPCProtocol, T>) -> Result<Self, Self::Error> {
        match value {
            VmYield::Result(results, _) => Ok(EncodedYield::Result(results.encode()?)),
            VmYield::Messages(messages) => Ok(EncodedYield::Messages(messages)),
            VmYield::Empty => Ok(EncodedYield::Empty),
        }
    }
}

pub(crate) struct ExecutionVmIo {
    pub(crate) compute_id: Uuid,
    pub(crate) results_service: Arc<dyn ResultsService>,
}

#[async_trait]
impl StateMachineIo for ExecutionVmIo {
    type StateMachineMessage = MPCExecutionVmMessage;
    type OutputMessage = ComputeStreamMessage;
    type Result = HashMap<String, NadaValue<Encrypted<Encoded>>>;
    type Metadata = StateMetadata;

    async fn open_party_stream(
        &self,
        channels: &dyn ClusterChannels,
        party_id: &PartyId,
    ) -> tonic::Result<Sender<ComputeStreamMessage>> {
        let initial_message = ComputeStreamMessage {
            compute_id: self.compute_id.as_bytes().to_vec(),
            bincode_message: vec![],
            compute_type: ComputeType::General.into(),
        };
        channels.open_compute_stream(party_id, initial_message).await
    }

    async fn handle_final_result(&self, result: anyhow::Result<(Self::Result, Self::Metadata)>) {
        let result = result.and_then(|(outputs, metadata)| metadata.split_outputs(outputs));
        let result = match result {
            Ok(values) => ComputeResult::Success { values },
            Err(e) => {
                error!("Failed to run compute: {e}");
                ComputeResult::Failure { error: e.to_string() }
            }
        };
        if let Err(e) = self.results_service.store_result(self.compute_id, result).await {
            error!("Failed to persist results: {e}");
        }
    }
}

impl StateMachineMessage<ComputeStreamMessage> for MPCExecutionVmMessage {
    fn try_encode(&self) -> anyhow::Result<Vec<u8>> {
        MessageCodec.encode(self).context("serializing message")
    }

    fn try_decode(bytes: &[u8]) -> anyhow::Result<Self> {
        MessageCodec.decode(bytes).context("deserializing message")
    }

    fn encoded_bytes_as_output_message(message: Vec<u8>) -> ComputeStreamMessage {
        ComputeStreamMessage { compute_id: vec![], bincode_message: message, compute_type: ComputeType::General.into() }
    }
}

#[derive(Clone)]
pub(crate) struct StateMetadata {
    pub(crate) user_outputs: Vec<UserOutputs>,
}

impl StateMetadata {
    pub(crate) fn new(
        bindings: Vec<OutputPartyBinding>,
        party_outputs: HashMap<&String, Vec<&Output>>,
    ) -> anyhow::Result<Self> {
        let mut user_outputs: HashMap<UserId, Vec<String>> = HashMap::new();
        for binding in bindings {
            let Some(outputs) = party_outputs.get(&binding.party_name) else {
                // we already validated this so this should never happen
                bail!("output party not defined");
            };
            // group all outputs for this user
            for user in binding.users {
                user_outputs.entry(user).or_default().extend(outputs.iter().map(|o| o.name.clone()));
            }
        }
        // translate it to what the compute operation expects
        let user_outputs = user_outputs.into_iter().map(|(user, outputs)| UserOutputs { user, outputs }).collect();
        Ok(Self { user_outputs })
    }

    pub(crate) fn split_outputs(
        self,
        outputs: HashMap<String, NadaValue<Encrypted<Encoded>>>,
    ) -> anyhow::Result<HashMap<UserId, Vec<NamedValue>>> {
        let mut protobuf_values = HashMap::new();
        for user_output in self.user_outputs {
            let mut values = HashMap::new();
            for output in user_output.outputs {
                let Some(value) = outputs.get(&output).cloned() else {
                    // this is a bug when setting compute up
                    bail!("output {output} has no defined output parties");
                };
                values.insert(output, value);
            }
            let values = nada_values_to_protobuf(values)?;
            protobuf_values.insert(user_output.user, values);
        }
        Ok(protobuf_values)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        channels::Party,
        services::{
            blob::DefaultBlobService,
            results::{DefaultResultsService, OutputPartyResult},
        },
        stateful::{
            builder::{DefaultPrimeBuilder, PrimeBuilder},
            sm::{BoxStateMachine, StateMachineArgs, StateMachineRunner},
            utils::{InitializeStateMachine, InitializedParty, Message, StateMachineSimulator},
            STREAM_CHANNEL_SIZE,
        },
        storage::{repositories::blob_expirations::SqliteBlobExpirationsRepository, sqlite::SqliteDb},
    };
    use basic_types::jar::PartyJar;
    use futures::executor::block_on;
    use math_lib::modular::{EncodedModulo, U64SafePrime};
    use mpc_vm::Program;
    use nada_value::{
        clear::Clear,
        encrypted::{nada_values_clear_to_nada_values_encrypted, nada_values_encrypted_to_nada_values_clear},
        protobuf::nada_values_from_protobuf,
        NadaType,
    };
    use shamir_sharing::secret_sharer::PartyShares;
    use std::time::Duration;
    use test_programs::PROGRAMS;
    use tokio::sync::mpsc::{channel, Receiver};
    use tokio_stream::{wrappers::ReceiverStream, StreamExt};
    use tracing_test::traced_test;

    struct ExecutionVmInitializer {
        program: Program<MPCProtocol>,
        inputs: HashMap<String, NadaValue<Clear>>,
        user_outputs: Vec<UserOutputs>,
        results_services: HashMap<PartyId, Arc<dyn ResultsService>>,
    }

    impl ExecutionVmInitializer {
        fn new(program: Program<MPCProtocol>, inputs: &[(&str, NadaValue<Clear>)], output_user: UserId) -> Self {
            let inputs = inputs.into_iter().map(|(k, v)| (k.to_string(), v.clone())).collect();
            let user_outputs = vec![UserOutputs {
                user: output_user,
                outputs: program.contract.outputs.iter().map(|o| o.name.clone()).clone().collect(),
            }];
            Self { program, inputs, user_outputs, results_services: Default::default() }
        }
    }

    impl<T> InitializeStateMachine<T, ExecutionVmIo> for ExecutionVmInitializer
    where
        T: SafePrime,
        ShamirSecretSharer<T>: SafePrimeSecretSharer<T>,
    {
        fn build_state_machines(
            &self,
            parties: Vec<Party>,
            sharers: &HashMap<PartyId, ShamirSecretSharer<T>>,
        ) -> HashMap<PartyId, BoxStateMachine<ExecutionVmIo>> {
            let parties: Vec<_> = parties.iter().map(|p| p.party_id.clone()).collect();
            let secret_sharer = sharers.values().next().expect("no sharers");
            let mut inputs: HashMap<_, _> =
                nada_values_clear_to_nada_values_encrypted(self.inputs.clone(), secret_sharer)
                    .expect("masking failed")
                    .into_elements()
                    .collect();
            let mut vms = HashMap::new();
            let compute_id = Uuid::new_v4();
            for party in &parties {
                let secret_sharer = sharers.get(&party).expect("sharer not found").clone();
                let inputs = inputs.remove(&party).expect("party inputs not found");
                let inputs = inputs.encode().expect("encoding failed");
                // TODO: support preprocessing elements
                let vm = DefaultPrimeBuilder::<T>::new(secret_sharer, Default::default())
                    .build_execution_vm(self.program.clone(), inputs, Default::default(), compute_id)
                    .expect("failed to build VM");
                vms.insert(party.clone(), vm);
            }
            vms
        }

        fn initialize_party(
            &mut self,
            compute_id: Uuid,
            party: PartyId,
            channels: Arc<dyn ClusterChannels>,
            state_machine: BoxStateMachine<ExecutionVmIo>,
        ) -> InitializedParty<ExecutionVmIo> {
            let db = block_on(async { SqliteDb::new("sqlite::memory:").await.expect("repo creation failed") });
            let expirations_repo = Arc::new(SqliteBlobExpirationsRepository::new(db));
            let results_service: Arc<dyn ResultsService> =
                Arc::new(DefaultResultsService::new(Box::new(DefaultBlobService::new_in_memory()), expirations_repo));
            self.results_services.insert(party.clone(), results_service.clone());

            let io = ExecutionVmIo { compute_id, results_service: results_service.clone() };
            let args = StateMachineArgs {
                id: compute_id,
                our_party_id: party.clone(),
                channels,
                timeout: Duration::from_secs(1),
                name: "COMPUTE",
                io,
                handles: Default::default(),
                cancel_token: Default::default(),
            };
            let handle = StateMachineRunner::start(args);
            let metadata = StateMetadata { user_outputs: self.user_outputs.clone() };
            InitializedParty { handle, state_machine, metadata }
        }

        fn transform_input_stream(&self, input: Receiver<Message>) -> Receiver<MPCExecutionVmMessage> {
            let (tx, rx) = channel(STREAM_CHANNEL_SIZE);
            let mut input = ReceiverStream::new(input);
            tokio::spawn(async move {
                while let Some(msg) = input.next().await {
                    let Message::Compute(msg) = msg else { panic!("not a compute message") };
                    // ignore the first signalling message
                    if msg.bincode_message.is_empty() {
                        continue;
                    }
                    let msg = MessageCodec.decode(&msg.bincode_message).expect("serde failed");
                    tx.send(msg).await.expect("send failed");
                }
            });
            rx
        }
    }

    #[tokio::test]
    #[traced_test]
    async fn program_execution() {
        let program = PROGRAMS.program("simple_shares").expect("program not found").0;
        let user = UserId::from_bytes("bob");
        let mut initializer = ExecutionVmInitializer::new(
            program,
            &[
                ("I00", NadaValue::new_secret_unsigned_integer(1u32)),
                ("I01", NadaValue::new_secret_unsigned_integer(2u32)),
                ("I02", NadaValue::new_secret_unsigned_integer(3u32)),
                ("I03", NadaValue::new_secret_unsigned_integer(4u32)),
                ("I04", NadaValue::new_secret_unsigned_integer(5u32)),
            ],
            user.clone(),
        );

        let runner = StateMachineSimulator::<U64SafePrime>::run(3, &mut initializer).await;
        for (party, handle) in runner.join_handles {
            println!("Waiting for {party} to finish execution");
            handle.await.expect("join failed");
        }
        let mut results = PartyShares::default();
        for (party, service) in initializer.results_services {
            let outputs =
                service.fetch_output_party_result(runner.identifier, &user).await.expect("failed to get output");
            let outputs: HashMap<String, NadaValue<Encrypted<Encoded>>> = match outputs {
                OutputPartyResult::Success { values } => {
                    nada_values_from_protobuf(values, &EncodedModulo::U64SafePrime).expect("failed to decode")
                }
                OutputPartyResult::Failure { error } => panic!("execution failed: {error}"),
            };
            results.insert(party, outputs);
        }
        let results = PartyJar::new_with_elements(results).unwrap();
        let results =
            nada_values_encrypted_to_nada_values_clear(results, &runner.secret_sharer).expect("reconstruction failed");
        assert_eq!(results, HashMap::from([("Add0".to_string(), NadaValue::new_secret_unsigned_integer(26u32))]));
    }

    #[test]
    fn split_outputs() {
        fn make_output(name: &str) -> Output {
            Output { name: name.to_string(), party: 0, ty: NadaType::Integer }
        }

        let bindings = vec![
            OutputPartyBinding {
                party_name: "A".into(),
                users: vec![UserId::from_bytes("U1"), UserId::from_bytes("U2")],
            },
            OutputPartyBinding {
                party_name: "B".into(),
                users: vec![UserId::from_bytes("U2"), UserId::from_bytes("U3")],
            },
            OutputPartyBinding { party_name: "C".into(), users: vec![UserId::from_bytes("U4")] },
        ];
        let outputs = HashMap::from([
            ("A".to_string(), vec![make_output("O1"), make_output("O2"), make_output("O3")]),
            ("B".to_string(), vec![make_output("O4"), make_output("O5")]),
            ("C".to_string(), vec![make_output("O6")]),
        ]);
        let outputs_ref = outputs.iter().map(|(k, v)| (k, v.iter().collect())).collect();
        let StateMetadata { mut user_outputs } = StateMetadata::new(bindings, outputs_ref).expect("creation failed");
        assert_eq!(user_outputs.len(), 4);
        user_outputs.sort_by_key(|u| u.user.clone());

        let users = user_outputs.iter().map(|u| &u.user).collect::<Vec<_>>();
        assert_eq!(
            &users,
            &[
                &UserId::from_bytes("U1"),
                &UserId::from_bytes("U2"),
                &UserId::from_bytes("U3"),
                &UserId::from_bytes("U4")
            ]
        );
        // U1 only gets party A's declared outputs
        assert_eq!(&user_outputs[0].outputs, &["O1", "O2", "O3"]);

        // U2 gets party A+B's declared outputs
        assert_eq!(&user_outputs[1].outputs, &["O1", "O2", "O3", "O4", "O5"]);

        // U3 gets party B's declared outputs
        assert_eq!(&user_outputs[2].outputs, &["O4", "O5"]);

        // U3 gets party C's declared outputs
        assert_eq!(&user_outputs[3].outputs, &["O6"]);
    }
}
