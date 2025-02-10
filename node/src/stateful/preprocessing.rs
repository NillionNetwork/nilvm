use super::sm::{
    EncodeableOutput, EncodedYield, StandardStateMachineState, StateMachine, StateMachineIo, StateMachineMessage,
};
use crate::{channels::ClusterChannels, services::preprocessing::PreprocessingBlobService};
use anyhow::{anyhow, bail, Context};
use async_trait::async_trait;
use basic_types::{PartyId, PartyMessage};
use encoding::codec::MessageCodec;
use math_lib::modular::{EncodedModularNumber, ModularNumber, SafePrime};
use node_api::{
    preprocessing::{
        proto::stream::PreprocessingStreamMessage,
        rust::{GeneratePreprocessingResponse, PreprocessingElement, PreprocessingProtocolStatus},
    },
    ConvertProto,
};
use protocols::{
    conditionals::{
        equality::offline::{
            output::PrepPrivateOutputEqualityShares, EncodedPrepPrivateOutputEqualityShares,
            PrepPrivateOutputEqualityStateOutput,
        },
        equality_public_output::offline::{
            EncodedPrepPublicOutputEqualityShares, PrepPublicOutputEqualityShares, PrepPublicOutputEqualityStateOutput,
        },
        less_than::offline::{EncodedPrepCompareShares, PrepCompareShares, PrepCompareStateOutput},
    },
    division::{
        division_secret_divisor::offline::{
            EncodedPrepDivisionIntegerSecretShares, PrepDivisionIntegerSecretShares,
            PrepDivisionIntegerSecretStateOutput,
        },
        modulo2m_public_divisor::offline::{EncodedPrepModulo2mShares, PrepModulo2mShares, PrepModulo2mStateOutput},
        modulo_public_divisor::offline::{EncodedPrepModuloShares, PrepModuloShares, PrepModuloStateOutput},
        truncation_probabilistic::offline::{EncodedPrepTruncPrShares, PrepTruncPrShares, PrepTruncPrStateOutput},
    },
    random::random_bit::{BitShare, EncodedBitShare, RandomBitStateOutput},
};
use serde::{de::DeserializeOwned, Serialize};
use std::{marker::PhantomData, sync::Arc};
use tokio::sync::mpsc::Sender;
use tracing::{error, info, warn};
use uuid::Uuid;

pub(crate) struct PreprocessingStateMachineIo<S>
where
    S: StandardStateMachineState<PreprocessingStreamMessage>,
{
    pub(crate) generation_id: Uuid,
    blob_service: Arc<dyn PreprocessingBlobService<<S::FinalResult as EncodeableOutput>::Output>>,
    element: PreprocessingElement,
    _unused: PhantomData<S>,
}

impl<S: StandardStateMachineState<PreprocessingStreamMessage>> PreprocessingStateMachineIo<S> {
    pub(crate) fn new(
        generation_id: Uuid,
        blob_service: Arc<dyn PreprocessingBlobService<<S::FinalResult as EncodeableOutput>::Output>>,
        element: PreprocessingElement,
    ) -> Self {
        Self { generation_id, blob_service, element, _unused: Default::default() }
    }
}

#[async_trait]
impl<S> StateMachineIo for PreprocessingStateMachineIo<S>
where
    S: StandardStateMachineState<PreprocessingStreamMessage>,
{
    type StateMachineMessage = S::OutputMessage;
    type OutputMessage = PreprocessingStreamMessage;
    type Result = anyhow::Result<Vec<<S::FinalResult as EncodeableOutput>::Output>>;
    type Metadata = PreprocessingMetadata;

    async fn open_party_stream(
        &self,
        channels: &dyn ClusterChannels,
        party_id: &PartyId,
    ) -> tonic::Result<Sender<PreprocessingStreamMessage>> {
        let initial_message = node_api::preprocessing::rust::PreprocessingStreamMessage {
            generation_id: self.generation_id.as_bytes().to_vec(),
            element: self.element,
            bincode_message: vec![],
        }
        .into_proto();
        channels.open_preprocessing_stream(party_id, initial_message).await
    }

    async fn handle_final_result(&self, result: anyhow::Result<(Self::Result, Self::Metadata)>) {
        // flatten the inner result
        let result = result.and_then(|(r, m)| r.map(|r| (r, m)));
        match result {
            Ok((shares, metadata)) => {
                let status = if let Err(e) = self.blob_service.upsert(metadata.batch_id as u32, shares).await {
                    error!("Failed to store result: {e}");
                    PreprocessingProtocolStatus::FinishedFailure
                } else {
                    info!("Result persisted successfully");
                    PreprocessingProtocolStatus::FinishedSuccess
                };
                let response = GeneratePreprocessingResponse { status }.into_proto();
                if metadata.response_channel.send(Ok(response)).await.is_err() {
                    warn!("Leader channel dropped before we could send response");
                }
            }
            Err(e) => {
                warn!("Preprocessing execution failed: {e}");
            }
        };
    }
}

pub(crate) struct PreprocessingMetadata {
    pub(crate) response_channel:
        Sender<tonic::Result<node_api::preprocessing::proto::generate::GeneratePreprocessingResponse>>,
    pub(crate) batch_id: u64,
}

impl<T> StateMachineMessage<PreprocessingStreamMessage> for T
where
    T: Serialize + DeserializeOwned + Clone + Send,
{
    fn try_encode(&self) -> anyhow::Result<Vec<u8>> {
        MessageCodec.encode(self).context("serializing message")
    }

    fn try_decode(bytes: &[u8]) -> anyhow::Result<Self> {
        MessageCodec.decode(bytes).context("deserializing message")
    }

    fn encoded_bytes_as_output_message(bincode_message: Vec<u8>) -> PreprocessingStreamMessage {
        // generation id and element are only necessary on the first message
        node_api::preprocessing::rust::PreprocessingStreamMessage {
            generation_id: vec![],
            element: PreprocessingElement::Compare,
            bincode_message,
        }
        .into_proto()
    }
}

pub(crate) struct FakePreprocessingStateMachine<R, M> {
    _unused: PhantomData<(R, M)>,
}

impl<R, M> Default for FakePreprocessingStateMachine<R, M> {
    fn default() -> Self {
        Self { _unused: Default::default() }
    }
}

impl<R, M> StateMachine for FakePreprocessingStateMachine<R, M>
where
    R: Send,
    M: Send,
{
    type Result = anyhow::Result<Vec<R>>;
    type Message = M;

    fn initialize(&mut self) -> anyhow::Result<EncodedYield<Self::Result, Self::Message>> {
        Ok(EncodedYield::Result(Ok(vec![])))
    }

    fn proceed(&mut self, _: PartyMessage<Self::Message>) -> anyhow::Result<EncodedYield<Self::Result, Self::Message>> {
        bail!("fake state machines cannot proceeed");
    }
}

macro_rules! impl_encodeable_output {
    ($raw_share:ty, $encoded_share:ty, $this:ident, $bound:tt) => {
        impl<T: $bound> EncodeableOutput for $this<$raw_share> {
            type Output = $encoded_share;

            fn encode(&self) -> anyhow::Result<Vec<Self::Output>> {
                match self.encode()? {
                    $this::Success { shares } => Ok(shares),
                    #[allow(unreachable_patterns)]
                    _ => Err(anyhow!("protocol failed")),
                }
            }
        }
    };
}

impl_encodeable_output!(PrepCompareShares<T>, EncodedPrepCompareShares, PrepCompareStateOutput, SafePrime);
impl_encodeable_output!(
    PrepDivisionIntegerSecretShares<T>,
    EncodedPrepDivisionIntegerSecretShares,
    PrepDivisionIntegerSecretStateOutput,
    SafePrime
);
impl_encodeable_output!(PrepModuloShares<T>, EncodedPrepModuloShares, PrepModuloStateOutput, SafePrime);
impl_encodeable_output!(
    PrepPublicOutputEqualityShares<T>,
    EncodedPrepPublicOutputEqualityShares,
    PrepPublicOutputEqualityStateOutput,
    SafePrime
);
impl_encodeable_output!(
    PrepPrivateOutputEqualityShares<T>,
    EncodedPrepPrivateOutputEqualityShares,
    PrepPrivateOutputEqualityStateOutput,
    SafePrime
);
impl_encodeable_output!(PrepTruncPrShares<T>, EncodedPrepTruncPrShares, PrepTruncPrStateOutput, SafePrime);
impl_encodeable_output!(PrepModulo2mShares<T>, EncodedPrepModulo2mShares, PrepModulo2mStateOutput, SafePrime);

impl<T: SafePrime> EncodeableOutput for Vec<ModularNumber<T>> {
    type Output = EncodedModularNumber;

    fn encode(&self) -> anyhow::Result<Vec<Self::Output>> {
        Ok(self.iter().map(|modular_number| modular_number.encode()).collect())
    }
}

impl<T: SafePrime> EncodeableOutput for RandomBitStateOutput<BitShare<T>> {
    type Output = EncodedBitShare;

    fn encode(&self) -> anyhow::Result<Vec<Self::Output>> {
        match self.encode() {
            RandomBitStateOutput::Success { shares } => Ok(shares),
            #[allow(unreachable_patterns)]
            _ => Err(anyhow!("protocol failed")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        services::{
            blob::DefaultBlobService, preprocessing::DefaultPreprocessingBlobService,
            runtime_elements::PreprocessingElementOffsets,
        },
        stateful::{
            builder::DefaultPrimeBuilder,
            sm::{BoxStateMachine, StateMachineArgs, StateMachineRunner},
            utils::{InitializeStateMachine, InitializedParty, Message, StateMachineSimulator},
            SIGNAL_CHANNEL_SIZE, STREAM_CHANNEL_SIZE,
        },
    };
    use math_lib::modular::{SafePrime, U64SafePrime};
    use rstest::rstest;
    use serde::de::DeserializeOwned;
    use shamir_sharing::secret_sharer::{SafePrimeSecretSharer, ShamirSecretSharer};
    use std::{collections::HashMap, time::Duration};
    use tokio::sync::mpsc::{channel, Receiver};
    use tokio_stream::{wrappers::ReceiverStream, StreamExt};

    type CreateStateMachine<T, S> =
        fn(DefaultPrimeBuilder<T>) -> anyhow::Result<BoxStateMachine<PreprocessingStateMachineIo<S>>>;

    struct PreprocessingInitializer<T, S>
    where
        T: SafePrime,
        S: StandardStateMachineState<PreprocessingStreamMessage>,
    {
        blob_services:
            HashMap<PartyId, Arc<dyn PreprocessingBlobService<<S::FinalResult as EncodeableOutput>::Output>>>,
        create_state_machine: CreateStateMachine<T, S>,
    }

    impl<T, S> PreprocessingInitializer<T, S>
    where
        T: SafePrime,
        S: StandardStateMachineState<PreprocessingStreamMessage>,
    {
        fn new(create_state_machine: CreateStateMachine<T, S>) -> Self {
            Self { blob_services: Default::default(), create_state_machine }
        }
    }

    impl<T, S> InitializeStateMachine<T, PreprocessingStateMachineIo<S>> for PreprocessingInitializer<T, S>
    where
        T: SafePrime,
        ShamirSecretSharer<T>: SafePrimeSecretSharer<T>,
        S: StandardStateMachineState<PreprocessingStreamMessage>,
        <S::FinalResult as EncodeableOutput>::Output: Clone + Serialize + DeserializeOwned + Send + Sync + 'static,
    {
        fn build_state_machines(
            &self,
            parties: Vec<crate::channels::Party>,
            sharers: &HashMap<PartyId, shamir_sharing::secret_sharer::ShamirSecretSharer<T>>,
        ) -> HashMap<PartyId, BoxStateMachine<PreprocessingStateMachineIo<S>>> {
            let mut state_machines = HashMap::new();
            for party in parties {
                let sharer = sharers.get(&party.party_id).expect("secret sharer not defined");
                let builder = DefaultPrimeBuilder::<T>::new(sharer.clone(), Default::default());
                let sm = (self.create_state_machine)(builder).expect("building state machine failed");
                state_machines.insert(party.party_id, sm);
            }
            state_machines
        }

        fn initialize_party(
            &mut self,
            generation_id: Uuid,
            party: PartyId,
            channels: Arc<dyn ClusterChannels>,
            state_machine: BoxStateMachine<PreprocessingStateMachineIo<S>>,
        ) -> InitializedParty<PreprocessingStateMachineIo<S>> {
            let blob_service: Arc<dyn PreprocessingBlobService<<S::FinalResult as EncodeableOutput>::Output>> =
                Arc::new(DefaultPreprocessingBlobService::new(Box::new(DefaultBlobService::new_in_memory())));
            self.blob_services.insert(party.clone(), blob_service.clone());

            let io = PreprocessingStateMachineIo {
                generation_id,
                element: PreprocessingElement::Compare,
                blob_service: blob_service.clone(),
                _unused: Default::default(),
            };
            let args = StateMachineArgs {
                id: generation_id,
                our_party_id: party.clone(),
                channels,
                timeout: Duration::from_secs(1),
                name: "PREPROCESSING",
                io,
                handles: Default::default(),
                cancel_token: Default::default(),
            };
            let handle = StateMachineRunner::start(args);
            let (sender, receiver) = channel(SIGNAL_CHANNEL_SIZE);
            tokio::spawn(async move {
                let mut receiver = ReceiverStream::new(receiver);
                while receiver.next().await.is_some() {}
            });
            let metadata = PreprocessingMetadata { response_channel: sender, batch_id: 0 };
            InitializedParty { handle, state_machine, metadata }
        }

        fn transform_input_stream(
            &self,
            input: Receiver<crate::stateful::utils::Message>,
        ) -> Receiver<<PreprocessingStateMachineIo<S> as StateMachineIo>::StateMachineMessage> {
            let (tx, rx) = channel(STREAM_CHANNEL_SIZE);
            let mut input = ReceiverStream::new(input);
            tokio::spawn(async move {
                while let Some(msg) = input.next().await {
                    let Message::Preprocessing(msg) = msg else { panic!("not a compute message") };
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

    struct GenerationMeta<S: StandardStateMachineState<PreprocessingStreamMessage>> {
        creator: CreateStateMachine<U64SafePrime, S>,
    }

    mod meta {
        use super::GenerationMeta;
        use crate::stateful::builder::PrimeBuilder;
        use math_lib::modular::U64SafePrime;
        use protocols::{
            conditionals::{
                equality::offline::PrepPrivateOutputEqualityState,
                equality_public_output::offline::state::PrepPublicOutputEqualityState,
                less_than::offline::state::PrepCompareState,
            },
            division::{
                division_secret_divisor::offline::state::PrepDivisionIntegerSecretState,
                modulo2m_public_divisor::offline::state::PrepModulo2mState,
                modulo_public_divisor::offline::state::PrepModuloState,
                truncation_probabilistic::offline::state::PrepTruncPrState,
            },
            random::{random_bit::RandomBitState, random_integer::RandomIntegerState},
        };

        pub(super) fn prep_compare() -> GenerationMeta<PrepCompareState<U64SafePrime>> {
            GenerationMeta { creator: |builder| builder.build_prep_compare_state_machine(1) }
        }

        pub(super) fn prep_division_secret_divisor() -> GenerationMeta<PrepDivisionIntegerSecretState<U64SafePrime>> {
            GenerationMeta { creator: |builder| builder.build_prep_division_secret_divisor_state_machine(1) }
        }

        pub(super) fn prep_modulo() -> GenerationMeta<PrepModuloState<U64SafePrime>> {
            GenerationMeta { creator: |builder| builder.build_prep_modulo_state_machine(1) }
        }

        pub(super) fn prep_equality_public_output() -> GenerationMeta<PrepPublicOutputEqualityState<U64SafePrime>> {
            GenerationMeta { creator: |builder| builder.build_prep_equality_public_output_state_machine(1) }
        }

        pub(super) fn prep_equality_secret_output() -> GenerationMeta<PrepPrivateOutputEqualityState<U64SafePrime>> {
            GenerationMeta { creator: |builder| builder.build_prep_equality_secret_output_state_machine(1) }
        }

        pub(super) fn prep_trunc_pr() -> GenerationMeta<PrepTruncPrState<U64SafePrime>> {
            GenerationMeta { creator: |builder| builder.build_prep_trunc_pr_state_machine(1) }
        }

        pub(super) fn prep_trunc() -> GenerationMeta<PrepModulo2mState<U64SafePrime>> {
            GenerationMeta { creator: |builder| builder.build_prep_trunc_state_machine(1) }
        }

        pub(super) fn random_integer() -> GenerationMeta<RandomIntegerState<U64SafePrime>> {
            GenerationMeta { creator: |builder| builder.build_random_integer_state_machine(1) }
        }

        pub(super) fn random_boolean() -> GenerationMeta<RandomBitState<U64SafePrime>> {
            GenerationMeta { creator: |builder| builder.build_random_boolean_state_machine(1) }
        }
    }

    #[rstest]
    #[case::compare(meta::prep_compare())]
    #[case::division_secret_divisor(meta::prep_division_secret_divisor())]
    #[case::modulo(meta::prep_modulo())]
    #[case::equality_public_output(meta::prep_equality_public_output())]
    #[case::equality_secret_output(meta::prep_equality_secret_output())]
    #[case::trunc_pr(meta::prep_trunc_pr())]
    #[case::trunc(meta::prep_trunc())]
    #[case::random_integer(meta::random_integer())]
    #[case::random_boolean(meta::random_boolean())]
    #[tokio::test]
    async fn generation<S>(#[case] meta: GenerationMeta<S>)
    where
        S: StandardStateMachineState<PreprocessingStreamMessage>,
        <S::FinalResult as EncodeableOutput>::Output:
            Clone + Serialize + DeserializeOwned + Send + Sync + 'static + std::fmt::Debug,
    {
        let mut initializer = PreprocessingInitializer::<U64SafePrime, S>::new(meta.creator);
        let runner = StateMachineSimulator::<U64SafePrime>::run(3, &mut initializer).await;
        for (party, handle) in runner.join_handles {
            println!("Waiting for {party} to finish execution");
            handle.await.expect("join failed");
        }
        // just ensure we can look them up
        for service in initializer.blob_services.values() {
            let offsets =
                PreprocessingElementOffsets { first_batch_id: 0, last_batch_id: 0, start_offset: 0, total: 1 };
            service.find_by_offsets(&offsets).await.expect("failed to get output");
        }
    }
}
