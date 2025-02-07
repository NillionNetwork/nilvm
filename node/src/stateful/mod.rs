use crate::channels::Party;
use anyhow::{bail, Context};
use futures::future;
use node_api::preprocessing::rust::PreprocessingProtocolStatus;
use std::mem;
use tokio_stream::{wrappers::ReceiverStream, StreamExt};
use tracing::info;

pub(crate) mod auxiliary_material;
pub(crate) mod auxiliary_material_scheduler;
pub(crate) mod builder;
pub(crate) mod cleanup;
pub(crate) mod compute;
pub(crate) mod preprocessing;
pub(crate) mod preprocessing_scheduler;
pub(crate) mod sm;

/// The size of channels used to signal events.
///
/// This is used typically for the channel used to signal something to the leader/client.
pub(crate) const SIGNAL_CHANNEL_SIZE: usize = 16;

/// The size of channels used to stream data between nodes.
///
/// This is used typically when running preprocessing/compute for the channel used to send the
/// protocol messages between peers.
pub(crate) const STREAM_CHANNEL_SIZE: usize = 256;

pub(crate) async fn wait_for_preprocessing_results<T, F>(
    mut pending_streams: Vec<(Party, ReceiverStream<tonic::Result<T>>)>,
    extractor: F,
) -> anyhow::Result<()>
where
    F: Fn(T) -> PreprocessingProtocolStatus,
{
    while !pending_streams.is_empty() {
        let mut futs = Vec::new();
        for (_, stream) in &mut pending_streams {
            futs.push(stream.next());
        }
        let results = future::join_all(futs).await;
        // iterate the pairs (result, originating stream) and keep only the non finished ones
        let results = results.into_iter().zip(mem::take(&mut pending_streams));
        for (result, (party, stream)) in results {
            let party_id = &party.party_id;
            let Some(result) = result else {
                bail!("Stream reached EOF before getting result from party {party_id}");
            };
            let status = extractor(result.context("waiting for generation status")?);
            match status {
                PreprocessingProtocolStatus::WaitingPeers => {
                    info!("Party {party_id} is waiting for peers");
                }
                PreprocessingProtocolStatus::FinishedSuccess => {
                    info!("Party {party_id} finished successfully");
                    continue;
                }
                PreprocessingProtocolStatus::FinishedFailure => {
                    bail!("Party {party_id} finished with an error");
                }
            };
            pending_streams.push((party, stream));
        }
    }
    Ok(())
}

#[cfg(test)]
mod utils {
    use super::{
        sm::{BoxStateMachine, InitMessage, StateMachineHandle, StateMachineIo},
        STREAM_CHANNEL_SIZE,
    };
    use crate::channels::{ClusterChannels, Party};
    use async_trait::async_trait;
    use basic_types::PartyId;
    use math_lib::modular::SafePrime;
    use node_api::{
        auth::rust::UserId,
        compute::rust::ComputeStreamMessage,
        preprocessing::{
            proto::stream::{AuxiliaryMaterialStreamMessage, PreprocessingStreamMessage},
            rust::{
                CleanupUsedElementsRequest, GenerateAuxiliaryMaterialRequest, GenerateAuxiliaryMaterialResponse,
                GeneratePreprocessingRequest, GeneratePreprocessingResponse,
            },
        },
    };
    use shamir_sharing::secret_sharer::{SafePrimeSecretSharer, ShamirSecretSharer};
    use std::{collections::HashMap, sync::Arc};
    use tokio::{
        sync::mpsc::{channel, Receiver, Sender},
        task::JoinHandle,
    };
    use tokio_stream::{wrappers::ReceiverStream, StreamExt};
    use tonic::Status;
    use uuid::Uuid;

    pub(crate) enum Message {
        Compute(ComputeStreamMessage),
        Preprocessing(PreprocessingStreamMessage),
        #[allow(dead_code)]
        AuxiliaryMaterial(AuxiliaryMaterialStreamMessage),
    }

    pub(crate) struct MemoryClusterChannels {
        parties: Vec<Party>,
        senders: HashMap<PartyId, Sender<Message>>,
    }

    impl MemoryClusterChannels {
        pub(crate) fn new(parties: Vec<Party>) -> (Self, HashMap<PartyId, Receiver<Message>>) {
            let mut senders = HashMap::new();
            let mut receivers = HashMap::new();
            for party in &parties {
                let (tx, rx) = channel(STREAM_CHANNEL_SIZE);
                receivers.insert(party.party_id.clone(), rx);
                senders.insert(party.party_id.clone(), tx);
            }
            (Self { parties, senders }, receivers)
        }
    }

    #[async_trait]
    impl ClusterChannels for MemoryClusterChannels {
        fn all_parties(&self) -> Vec<Party> {
            todo!()
        }

        fn other_parties(&self) -> Vec<Party> {
            self.parties.clone()
        }

        fn is_member(&self, _: &UserId) -> bool {
            true
        }

        async fn open_compute_stream(
            &self,
            party: &PartyId,
            initial_message: ComputeStreamMessage,
        ) -> tonic::Result<Sender<ComputeStreamMessage>> {
            let (sender, receiver) = channel(STREAM_CHANNEL_SIZE);
            let party_sender = self.senders.get(party).expect("party not registered").clone();
            party_sender
                .send(Message::Compute(initial_message))
                .await
                .map_err(|_| Status::internal("failed to send initial message"))?;

            let mut messages = ReceiverStream::new(receiver);
            tokio::spawn(async move {
                while let Some(msg) = messages.next().await {
                    party_sender.send(Message::Compute(msg)).await.expect("receiver dropped too early");
                }
            });
            Ok(sender)
        }

        async fn open_preprocessing_stream(
            &self,
            party: &PartyId,
            initial_message: PreprocessingStreamMessage,
        ) -> tonic::Result<Sender<PreprocessingStreamMessage>> {
            let (sender, receiver) = channel(STREAM_CHANNEL_SIZE);
            let party_sender = self.senders.get(party).expect("party not registered").clone();
            party_sender
                .send(Message::Preprocessing(initial_message))
                .await
                .map_err(|_| Status::internal("failed to send initial message"))?;

            let mut messages = ReceiverStream::new(receiver);
            tokio::spawn(async move {
                while let Some(msg) = messages.next().await {
                    party_sender.send(Message::Preprocessing(msg)).await.expect("receiver dropped too early");
                }
            });
            Ok(sender)
        }

        async fn open_auxiliary_material_stream(
            &self,
            party: &PartyId,
            initial_message: AuxiliaryMaterialStreamMessage,
        ) -> tonic::Result<Sender<AuxiliaryMaterialStreamMessage>> {
            let (sender, receiver) = channel(STREAM_CHANNEL_SIZE);
            let party_sender = self.senders.get(party).expect("party not registered").clone();
            party_sender
                .send(Message::AuxiliaryMaterial(initial_message))
                .await
                .map_err(|_| Status::internal("failed to send initial message"))?;

            let mut messages = ReceiverStream::new(receiver);
            tokio::spawn(async move {
                while let Some(msg) = messages.next().await {
                    party_sender.send(Message::AuxiliaryMaterial(msg)).await.expect("receiver dropped too early");
                }
            });
            Ok(sender)
        }

        async fn generate_preprocessing(
            &self,
            _party: PartyId,
            _request: GeneratePreprocessingRequest,
        ) -> tonic::Result<Receiver<tonic::Result<GeneratePreprocessingResponse>>> {
            todo!()
        }

        async fn generate_auxiliary_material(
            &self,
            _party: PartyId,
            _request: GenerateAuxiliaryMaterialRequest,
        ) -> tonic::Result<Receiver<tonic::Result<GenerateAuxiliaryMaterialResponse>>> {
            todo!()
        }

        async fn cleanup_used_elements(
            &self,
            _party: PartyId,
            _request: CleanupUsedElementsRequest,
        ) -> tonic::Result<()> {
            todo!()
        }
    }

    /// Allows initializing a state machine for testing purposes.
    pub(crate) trait InitializeStateMachine<T, I>
    where
        T: SafePrime,
        ShamirSecretSharer<T>: SafePrimeSecretSharer<T>,
        I: StateMachineIo,
    {
        /// Build the state machines for all parties.
        fn build_state_machines(
            &self,
            parties: Vec<Party>,
            sharers: &HashMap<PartyId, ShamirSecretSharer<T>>,
        ) -> HashMap<PartyId, BoxStateMachine<I>>;

        /// Initialize a specific party.
        fn initialize_party(
            &mut self,
            compute_id: Uuid,
            party: PartyId,
            channels: Arc<dyn ClusterChannels>,
            state_machine: BoxStateMachine<I>,
        ) -> InitializedParty<I>;

        /// Turn a stream of the more generic `Message` type into one that contains the specific type we expect.
        fn transform_input_stream(&self, input: Receiver<Message>) -> Receiver<I::StateMachineMessage>;
    }

    /// An initialized party.
    pub(crate) struct InitializedParty<I: StateMachineIo> {
        pub(crate) handle: StateMachineHandle<I>,
        pub(crate) state_machine: BoxStateMachine<I>,
        pub(crate) metadata: I::Metadata,
    }

    /// A state machine simulator.
    ///
    /// This allows running state machines as close to "the real thing" as possible. This sets up
    /// in memory channels between the different parties so that they can talk to each other almost
    /// in the same way as they would if they were using the gRPC API.
    pub(crate) struct StateMachineSimulator<T>
    where
        T: SafePrime,
        ShamirSecretSharer<T>: SafePrimeSecretSharer<T>,
    {
        pub(crate) identifier: Uuid,
        pub(crate) join_handles: Vec<(PartyId, JoinHandle<()>)>,
        pub(crate) secret_sharer: ShamirSecretSharer<T>,
    }

    impl<T> StateMachineSimulator<T>
    where
        T: SafePrime,
        ShamirSecretSharer<T>: SafePrimeSecretSharer<T>,
    {
        pub(crate) async fn run<I>(party_count: usize, initializer: &mut impl InitializeStateMachine<T, I>) -> Self
        where
            I: StateMachineIo,
        {
            let mut parties = Vec::new();
            // initialize parties
            for i in 0..party_count {
                let i = i.to_string();
                let party_id = PartyId::from(i.as_bytes().to_vec());
                let user_id = UserId::from_bytes(i);
                parties.push(Party { party_id, user_id });
            }
            // initialize all channels for parties
            let mut party_receivers = HashMap::new();
            let mut channels: HashMap<_, Arc<dyn ClusterChannels>> = HashMap::new();
            for party in &parties {
                // we don't want a channel to ourselves
                let parties = parties.iter().filter(|p| p.party_id != party.party_id).cloned().collect();
                let (party_channels, receivers) = MemoryClusterChannels::new(parties);
                channels.insert(party.party_id.clone(), Arc::new(party_channels));
                party_receivers.insert(party.party_id.clone(), receivers);
            }
            let secret_sharers = Self::build_secret_sharers(&parties);
            let mut vms = initializer.build_state_machines(parties.clone(), &secret_sharers);
            let secret_sharer = secret_sharers.into_values().next().unwrap();
            let identifier = Uuid::new_v4();
            let mut runner = Self { identifier, join_handles: Default::default(), secret_sharer };
            let mut init_senders = HashMap::new();
            // start every party
            for (party, channels) in channels {
                let state_machine = vms.remove(&party).expect("state machine not created");
                let InitializedParty { handle, state_machine: vm, metadata } =
                    initializer.initialize_party(identifier, party.clone(), channels, state_machine);
                handle
                    .init_sender
                    .send(InitMessage::InitStateMachine { state_machine: vm, metadata })
                    .await
                    .expect("sending vm failed");
                runner.join_handles.push((party.clone(), handle.join_handle));
                init_senders.insert(party, handle.init_sender);
            }
            // initialize each party by sending the pre-created receivers to each other
            for (party, receivers) in party_receivers {
                let user_id = parties.iter().find(|p| p.party_id == party).expect("party not found").user_id.clone();
                for (target_party, receiver) in receivers {
                    let init_sender = init_senders.get(&target_party).expect("party not found");
                    let stream = initializer.transform_input_stream(receiver);
                    init_sender
                        .send(InitMessage::InitParty { user_id: user_id.clone(), stream })
                        .await
                        .expect("init party failed");
                }
            }
            runner
        }

        fn build_secret_sharers(parties: &[Party]) -> HashMap<PartyId, ShamirSecretSharer<T>>
        where
            T: SafePrime,
            ShamirSecretSharer<T>: SafePrimeSecretSharer<T>,
        {
            let parties: Vec<_> = parties.iter().map(|p| p.party_id.clone()).collect();
            let poly_degree = 1;
            let mut sharers = HashMap::new();
            for party in &parties {
                let secret_sharer =
                    ShamirSecretSharer::<T>::new(party.clone(), poly_degree, parties.clone()).expect("creating sharer");
                sharers.insert(party.clone(), secret_sharer);
            }
            sharers
        }
    }
}
