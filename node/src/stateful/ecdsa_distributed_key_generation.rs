use super::sm::{StateMachineIo, StateMachineMessage};
use crate::{
    channels::ClusterChannels,
    services::{results::ResultsService, user_values::UserValuesService},
    stateful::{
        compute::{StateMetadata, UserOutputs},
        sm::EncodeableOutput,
    },
    storage::models::{result::ComputeResult, user_values::UserValuesRecord},
};
use anyhow::Context;
use async_trait::async_trait;
use basic_types::PartyId;
use chrono::{Duration, Utc};
use encoding::codec::MessageCodec;
use itertools::Itertools;
use nada_value::{
    encrypted::{Encoded, Encrypted},
    protobuf::nada_values_to_protobuf,
    NadaValue,
};
use node_api::{
    compute::{
        proto::stream::ComputeType,
        rust::{ComputeStreamMessage, OutputPartyBinding},
        TECDSA_PRIVATE_KEY_STORE_ID_PARTY, TECDSA_PUBLIC_KEY, TECDSA_PUBLIC_KEY_PARTY, TECDSA_SIGN_PROGRAM_ID,
        TECDSA_STORE_ID,
    },
    membership::rust::Prime,
    permissions::rust::{ComputePermission, Permissions},
};
use protocols::distributed_key_generation::dkg::{
    output::{EcdsaKeyGenOutput, EcdsaPrivateKeyShare},
    state::{EcdsaKeyGenStateMessage, KeyGenStateMessageType, RoundStateMessage},
};
use std::{
    collections::{BTreeSet, HashMap, HashSet},
    result::Result::Ok,
    sync::Arc,
};
use tokio::sync::mpsc::Sender;
use tonic::Status;
use tracing::error;
use uuid::Uuid;

const TEN_YEARS_IN_DAYS: i64 = 365 * 10;

pub(crate) struct EcdsaDistributedKeyGenerationIo {
    pub(crate) compute_id: Uuid,
    pub(crate) results_service: Arc<dyn ResultsService>,
    pub(crate) user_values_service: Arc<dyn UserValuesService>,
}

#[async_trait]
impl StateMachineIo for EcdsaDistributedKeyGenerationIo {
    type StateMachineMessage = EcdsaKeyGenStateMessage;
    type OutputMessage = ComputeStreamMessage;
    type Result = anyhow::Result<Vec<EcdsaKeyGenOutput>>;
    type Metadata = StateMetadata;

    async fn open_party_stream(
        &self,
        channels: &dyn ClusterChannels,
        party_id: &PartyId,
    ) -> tonic::Result<Sender<ComputeStreamMessage>> {
        let initial_message = ComputeStreamMessage {
            compute_id: self.compute_id.as_bytes().to_vec(),
            bincode_message: vec![],
            compute_type: ComputeType::EcdsaDkg.into(),
        };
        channels.open_compute_stream(party_id, initial_message).await
    }

    async fn handle_final_result(&self, result: anyhow::Result<(Self::Result, Self::Metadata)>) {
        // For ECDSA DKG, we store two outputs in the store_result service:
        //  - the store_id
        //  - the ecdsa_public_key.
        // The ecdsa_private_key shares are stored separately in the user values service since they must remain on each node.

        // Handle the result and store an error if it fails
        let (result, record) = match do_handle_final_result(self.compute_id, result) {
            Ok((result, record)) => (result, record),
            Err(e) => {
                error!("Failed to handle final result: {e}");
                let error_result = ComputeResult::Failure { error: e.to_string() };
                if let Err(e) = self.results_service.store_result(self.compute_id, error_result).await {
                    error!("Failed to persist results: {e}");
                }
                return;
            }
        };

        // Store the ecdsa private key shares in the user values service
        // If there is an error, store the error in the results service
        if let Err(e) = self.user_values_service.create_if_not_exists(self.compute_id, record).await {
            error!("Failed to store ecdsa private key in user values service: {e}");
            let error_result = ComputeResult::Failure { error: e.to_string() };
            if let Err(e) = self.results_service.store_result(self.compute_id, error_result).await {
                error!("Failed to persist results: {e}");
            }
            return;
        }

        // Store the result in the results service
        if let Err(e) = self.results_service.store_result(self.compute_id, result).await {
            error!("Failed to persist results: {e}");
        }
    }
}

// Auxiliary functions
fn do_handle_final_result(
    compute_id: Uuid,
    result: anyhow::Result<(
        <EcdsaDistributedKeyGenerationIo as StateMachineIo>::Result,
        <EcdsaDistributedKeyGenerationIo as StateMachineIo>::Metadata,
    )>,
) -> anyhow::Result<(ComputeResult, UserValuesRecord)> {
    let (result, metadata) = match result {
        Ok((Ok(mut key), metadata)) => (key.pop(), metadata),
        Err(e) | Ok((Err(e), _)) => return Err(anyhow::anyhow!("Failed to handle result of ECDSA DKG compute: {e}")),
    };
    let Some(EcdsaKeyGenOutput::Success { element }) = result else {
        return Err(anyhow::anyhow!("Failed to get ECDSA key output"));
    };

    // Create the ecdsa private key record
    let record = match create_private_key_record(&metadata, element.clone()) {
        Ok(result) => result,
        Err(e) => {
            return Err(anyhow::anyhow!("Failed to create private key record: {e}"));
        }
    };

    // Create store_id and extract ecdsa_public_key
    // The store_id value is defined as the compute_id because this needs to be identical between
    // all nodes in the cluster and its generation must be deterministic. Since we are only running
    // one ecdsa dkg protocol per compute_id, this is sufficient.
    let store_id_bytes: [u8; 16] = *compute_id.as_bytes();
    let encoded_public_key = element.as_inner().shared_public_key.to_bytes(true);
    let public_key_slice = encoded_public_key.as_bytes();
    let public_key: [u8; 33] =
        public_key_slice.try_into().map_err(|_| anyhow::anyhow!("Public key has incorrect length"))?;

    let values = HashMap::from([
        (TECDSA_STORE_ID.to_string(), NadaValue::<Encrypted<Encoded>>::new_store_id(store_id_bytes)),
        (TECDSA_PUBLIC_KEY.to_string(), NadaValue::<Encrypted<Encoded>>::new_ecdsa_public_key(public_key)),
    ]);
    // Convert the values HashMap to the expected type using split_outputs
    let result = match metadata.clone().split_outputs(values) {
        Ok(split_values) => ComputeResult::Success { values: split_values },
        Err(e) => return Err(anyhow::anyhow!("Failed to split outputs: {e}")),
    };

    Ok((result, record))
}

fn create_private_key_record(
    metadata: &StateMetadata,
    element: EcdsaPrivateKeyShare,
) -> anyhow::Result<UserValuesRecord> {
    let user_id = metadata
        .user_outputs
        .first()
        .map(|output| output.user)
        .ok_or_else(|| anyhow::anyhow!("No user outputs found"))?;

    let nada_values = HashMap::from([(
        "tecdsa_private_key".to_string(),
        NadaValue::<Encrypted<Encoded>>::new_ecdsa_private_key(element),
    )]);

    let values = nada_values_to_protobuf(nada_values).context("Failed to convert nada values to protobuf")?;

    let permissions = Permissions {
        owner: user_id,
        retrieve: HashSet::from([user_id]),
        update: HashSet::from([user_id]),
        delete: HashSet::from([user_id]),
        compute: HashMap::from([(
            user_id,
            ComputePermission { program_ids: HashSet::from([TECDSA_SIGN_PROGRAM_ID.to_string()]) },
        )]),
    };

    let expires_at = Utc::now()
        .checked_add_signed(Duration::days(TEN_YEARS_IN_DAYS))
        .context("Expiration date calculation overflowed")?;

    Ok(UserValuesRecord { values, permissions, expires_at, prime: Prime::Safe64Bits })
}

impl EncodeableOutput for EcdsaKeyGenOutput {
    type Output = EcdsaKeyGenOutput;

    fn encode(&self) -> anyhow::Result<Vec<EcdsaKeyGenOutput>> {
        Ok(vec![self.clone()])
    }
}

impl StateMachineMessage<ComputeStreamMessage> for EcdsaKeyGenStateMessage {
    fn try_encode(&self) -> anyhow::Result<Vec<u8>> {
        MessageCodec.encode(self).context("serializing message")
    }

    fn try_decode(bytes: &[u8]) -> anyhow::Result<Self> {
        MessageCodec.decode(bytes).context("deserializing message")
    }

    fn encoded_bytes_as_output_message(message: Vec<u8>) -> ComputeStreamMessage {
        ComputeStreamMessage {
            compute_id: vec![],
            bincode_message: message,
            compute_type: ComputeType::EcdsaDkg.into(),
        }
    }
}

impl StateMachineMessage<EcdsaKeyGenStateMessage> for EcdsaKeyGenStateMessage {
    fn try_encode(&self) -> anyhow::Result<Vec<u8>> {
        MessageCodec.encode(self).context("serializing message")
    }

    fn try_decode(bytes: &[u8]) -> anyhow::Result<Self> {
        MessageCodec.decode(bytes).context("deserializing message")
    }

    fn encoded_bytes_as_output_message(message: Vec<u8>) -> EcdsaKeyGenStateMessage {
        MessageCodec.decode(&message).unwrap_or_else(|_| {
            error!("Failed to decode message");
            EcdsaKeyGenStateMessage::Message(RoundStateMessage {
                msg: None,
                msg_type: KeyGenStateMessageType::Broadcast,
            })
        })
    }
}

pub(crate) fn create_user_outputs(output_bindings: &[OutputPartyBinding]) -> Result<Vec<UserOutputs>, Status> {
    // HashMap to store outputs for each user
    let mut user_outputs = HashMap::new();

    // Validate that all parties are bound.
    let mut missing_parties = BTreeSet::from([TECDSA_PRIVATE_KEY_STORE_ID_PARTY, TECDSA_PUBLIC_KEY_PARTY]);

    // Process each output binding
    for binding in output_bindings {
        // Determine outputs based on party name
        let outputs = match binding.party_name.as_str() {
            TECDSA_PRIVATE_KEY_STORE_ID_PARTY => vec![TECDSA_STORE_ID.to_string()],
            TECDSA_PUBLIC_KEY_PARTY => vec![TECDSA_PUBLIC_KEY.to_string()],
            _ => {
                return Err(Status::invalid_argument(format!(
                    "invalid output party name binding for DKG protocol execution: {}",
                    binding.party_name
                )));
            }
        };

        // Add outputs to each user's set of outputs
        for user in &binding.users {
            user_outputs.entry(user).or_insert_with(HashSet::new).extend(outputs.clone());
        }
        missing_parties.remove(binding.party_name.as_str());
    }

    if !missing_parties.is_empty() {
        let missing_parties = missing_parties.iter().join(", ");
        return Err(Status::invalid_argument(format!("required parties not bound: [{missing_parties}]")));
    }

    // Convert the map into the required UserOutputs format
    Ok(user_outputs
        .into_iter()
        .map(|(user, outputs)| UserOutputs { user: *user, outputs: outputs.into_iter().collect() })
        .collect())
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        channels::Party,
        services::{
            blob::DefaultBlobService,
            results::{DefaultResultsService, OutputPartyResult},
            user_values::DefaultUserValuesService,
        },
        stateful::{
            compute::UserOutputs,
            sm::{BoxStateMachine, StandardStateMachine, StateMachine, StateMachineArgs, StateMachineRunner},
            utils::{InitializeStateMachine, InitializedParty, Message, StateMachineSimulator},
            STREAM_CHANNEL_SIZE,
        },
        storage::{repositories::blob_expirations::SqliteBlobExpirationsRepository, sqlite::SqliteDb},
    };
    use basic_types::jar::PartyJar;
    use futures::executor::block_on;
    use math_lib::modular::{EncodedModulo, SafePrime, U64SafePrime};
    use nada_value::{encrypted::nada_values_encrypted_to_nada_values_clear, protobuf::nada_values_from_protobuf};
    use node_api::{auth::rust::UserId, compute::rust::OutputPartyBinding};
    use protocols::distributed_key_generation::dkg::EcdsaKeyGenState;
    use rstest::rstest;
    use shamir_sharing::secret_sharer::{PartyShares, SafePrimeSecretSharer, ShamirSecretSharer};
    use std::time::Duration;
    use tokio::sync::mpsc::{channel, Receiver};
    use tokio_stream::{wrappers::ReceiverStream, StreamExt};
    use tracing_test::traced_test;

    struct EcdsaDkgInitializer {
        user_outputs: Vec<UserOutputs>,
        results_services: HashMap<PartyId, Arc<dyn ResultsService>>,
        user_values_services: HashMap<PartyId, Arc<dyn UserValuesService>>,
    }

    impl EcdsaDkgInitializer {
        fn new(output_user: UserId) -> Self {
            let user_outputs = vec![UserOutputs {
                user: output_user,
                outputs: vec![TECDSA_STORE_ID.to_string(), TECDSA_PUBLIC_KEY.to_string()],
            }];
            Self { user_outputs, results_services: Default::default(), user_values_services: Default::default() }
        }
    }

    impl<T> InitializeStateMachine<T, EcdsaDistributedKeyGenerationIo> for EcdsaDkgInitializer
    where
        T: SafePrime,
        ShamirSecretSharer<T>: SafePrimeSecretSharer<T>,
    {
        fn build_state_machines(
            &self,
            parties: Vec<Party>,
            _sharers: &HashMap<PartyId, ShamirSecretSharer<T>>,
        ) -> HashMap<PartyId, BoxStateMachine<EcdsaDistributedKeyGenerationIo>> {
            let parties: Vec<_> = parties.iter().map(|p| p.party_id.clone()).collect();
            let mut vms = HashMap::new();
            for party in &parties {
                let eid = b"execution id, unique per protocol execution".to_vec();
                let (state, initial_messages) =
                    EcdsaKeyGenState::new(eid, parties.clone(), party.clone()).expect("state creation failed");
                let sm = state_machine::StateMachine::new(state);
                let sm = StandardStateMachine::<EcdsaKeyGenState, EcdsaKeyGenStateMessage>::new(sm, initial_messages);
                let vm: Box<
                    dyn StateMachine<Message = EcdsaKeyGenStateMessage, Result = anyhow::Result<Vec<EcdsaKeyGenOutput>>>,
                > = Box::new(sm);
                vms.insert(party.clone(), vm);
            }
            vms
        }

        fn initialize_party(
            &mut self,
            compute_id: Uuid,
            party: PartyId,
            channels: Arc<dyn ClusterChannels>,
            state_machine: BoxStateMachine<EcdsaDistributedKeyGenerationIo>,
        ) -> InitializedParty<EcdsaDistributedKeyGenerationIo> {
            let db = block_on(async { SqliteDb::new("sqlite::memory:").await.expect("repo creation failed") });
            let expirations_repo = Arc::new(SqliteBlobExpirationsRepository::new(db));
            // results service
            let results_service: Arc<dyn ResultsService> = Arc::new(DefaultResultsService::new(
                Box::new(DefaultBlobService::new_in_memory()),
                expirations_repo.clone(),
            ));
            self.results_services.insert(party.clone(), results_service.clone());

            // user values service
            let user_values_service: Arc<dyn UserValuesService> = Arc::new(DefaultUserValuesService::new(
                Box::new(DefaultBlobService::new_in_memory()),
                expirations_repo,
            ));
            self.user_values_services.insert(party.clone(), user_values_service.clone());

            let io = EcdsaDistributedKeyGenerationIo {
                compute_id,
                results_service: results_service.clone(),
                user_values_service: user_values_service.clone(),
            };
            let args = StateMachineArgs {
                id: compute_id,
                our_party_id: party.clone(),
                channels,
                timeout: Duration::from_secs(1),
                name: "ECDSA_DKG",
                io,
                handles: Default::default(),
                cancel_token: Default::default(),
            };
            let handle = StateMachineRunner::start(args);
            let metadata = StateMetadata { user_outputs: self.user_outputs.clone() };
            InitializedParty { handle, state_machine, metadata }
        }

        fn transform_input_stream(&self, input: Receiver<Message>) -> Receiver<EcdsaKeyGenStateMessage> {
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
        let user = UserId::from_bytes("bob");
        let mut initializer = EcdsaDkgInitializer::new(user.clone());

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
        let _results =
            nada_values_encrypted_to_nada_values_clear(results, &runner.secret_sharer).expect("reconstruction failed");
    }

    #[rstest]
    #[case::all(&[])]
    #[case::store_id(&[TECDSA_PRIVATE_KEY_STORE_ID_PARTY])]
    #[case::public_key(&[TECDSA_PUBLIC_KEY_PARTY])]
    fn missing_parties(#[case] parties: &[&str]) {
        let bindings: Vec<_> = parties
            .iter()
            .map(|p| OutputPartyBinding { party_name: p.to_string(), users: vec![UserId::from_bytes(b"")] })
            .collect();
        create_user_outputs(&bindings).expect_err("binding no parties succeeded");
    }
}
