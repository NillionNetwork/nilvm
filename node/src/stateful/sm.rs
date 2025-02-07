use crate::{
    channels::{ClusterChannels, Party},
    stateful::SIGNAL_CHANNEL_SIZE,
};
use anyhow::{anyhow, bail, Context};
use async_trait::async_trait;
use basic_types::{PartyId, PartyMessage};
use futures::{future::join_all, stream::select_all, Stream};
use metrics::prelude::*;
use node_api::auth::rust::UserId;
use once_cell::sync::Lazy;
use state_machine::state::{Recipient, RecipientMessage};
use std::{
    collections::HashMap,
    future::Future,
    marker::PhantomData,
    mem,
    ops::Deref,
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::{
    sync::{
        mpsc::{channel, error::SendError, Receiver, Sender},
        Mutex,
    },
    task::JoinHandle,
    time::timeout,
};
use tokio_stream::{wrappers::ReceiverStream, StreamExt};
use tokio_util::sync::CancellationToken;
use tracing::{error, info, info_span, warn, Instrument};
use uuid::Uuid;

static METRICS: Lazy<Metrics> = Lazy::new(Metrics::default);

pub(crate) trait StateMachine: Send {
    type Result;
    type Message;

    fn initialize(&mut self) -> anyhow::Result<EncodedYield<Self::Result, Self::Message>>;
    fn proceed(
        &mut self,
        message: PartyMessage<Self::Message>,
    ) -> anyhow::Result<EncodedYield<Self::Result, Self::Message>>;
}

pub(crate) enum EncodedYield<R, S> {
    Result(R),
    Messages(Vec<RecipientMessage<PartyId, S>>),
    Empty,
}

/// Defines the input/output abstraction for a state machine.
#[async_trait]
pub(crate) trait StateMachineIo: Send + Sync + 'static {
    /// The message the underlying state machine expects.
    type StateMachineMessage: StateMachineMessage<Self::OutputMessage>;

    /// The message used to communicate with the outside world. This typically maps to a protobuf
    /// message defined as part of the protocol's gRPC API.
    type OutputMessage: Send + Sync;

    /// The final result of the underlying state machine.
    type Result: Send;

    /// Metadata that's necessary to perform any final transformations to the final result.
    type Metadata: Send;

    /// Open a stream to the given party.
    ///
    /// This should invoke the specific API call that initializes a stream for this state machine's
    /// protocol.
    async fn open_party_stream(
        &self,
        channels: &dyn ClusterChannels,
        party_id: &PartyId,
    ) -> tonic::Result<Sender<Self::OutputMessage>>;

    /// Handle the final result and do anything that's necessary to persist/communicate that the
    /// state machine finished to whoever's necessary.
    async fn handle_final_result(&self, result: anyhow::Result<(Self::Result, Self::Metadata)>);
}

/// A state machine message.
pub(crate) trait StateMachineMessage<O>: Send + Clone {
    /// Try to encode this message into bytes.
    fn try_encode(&self) -> anyhow::Result<Vec<u8>>;

    /// Try to encode this message into bytes.
    fn try_decode(bytes: &[u8]) -> anyhow::Result<Self>;

    /// Create an output message out of the encoded state machine message.
    fn encoded_bytes_as_output_message(message: Vec<u8>) -> O;
}

/// A boxed state machine for the given [StateMachineIo].
pub(crate) type BoxStateMachine<I> =
    Box<dyn StateMachine<Message = <I as StateMachineIo>::StateMachineMessage, Result = <I as StateMachineIo>::Result>>;

/// A message that initializes a state machine.
pub(crate) enum InitMessage<I>
where
    I: StateMachineIo,
{
    // A message indicating we want to initialize a peer with the given id and which will send us
    // messages via the given `stream`.
    InitParty { user_id: UserId, stream: Receiver<I::StateMachineMessage> },

    // A message indicating we want to initialize the state machine. This should only be sent once.
    InitStateMachine { state_machine: BoxStateMachine<I>, metadata: I::Metadata },
}

/// A state machine handle that allows initializing and joining it.
pub(crate) struct StateMachineHandle<I>
where
    I: StateMachineIo,
{
    pub(crate) init_sender: Sender<InitMessage<I>>,
    #[allow(dead_code)]
    pub(crate) join_handle: JoinHandle<()>,
}

impl<I> StateMachineHandle<I>
where
    I: StateMachineIo,
{
    pub(crate) async fn send(&self, message: InitMessage<I>) -> Result<(), ChannelDropped> {
        self.init_sender.send(message).await.map_err(|_| ChannelDropped)
    }
}

/// A standard state machine which:
///
/// * Takes a `PartyMessage<M>` as input.
/// * Emits `M` as output.
/// * The final result can be encoded.
/// * Uses `PartyId` to identify recipients.
pub(crate) trait StandardStateMachineState<M>:
    state_machine::StateMachineState<
        RecipientId = PartyId,
        InputMessage = PartyMessage<<Self as state_machine::StateMachineState>::OutputMessage>,
        FinalResult: EncodeableOutput,
        OutputMessage: StateMachineMessage<M>,
    > + Send
    + Sync
    + 'static
{
}

impl<S, M> StandardStateMachineState<M> for S where
    S: state_machine::StateMachineState<
            RecipientId = PartyId,
            InputMessage = PartyMessage<<Self as state_machine::StateMachineState>::OutputMessage>,
            FinalResult: EncodeableOutput,
            OutputMessage: StateMachineMessage<M>,
        > + Send
        + Sync
        + 'static
{
}

pub(crate) struct StandardStateMachine<S: state_machine::StateMachineState, M> {
    sm: state_machine::StateMachine<S>,
    initial_messages: Vec<state_machine::state::StateMachineMessage<S>>,
    _unused: PhantomData<M>,
}

impl<S: state_machine::StateMachineState, M> StandardStateMachine<S, M> {
    pub(crate) fn new(
        sm: state_machine::StateMachine<S>,
        initial_messages: Vec<state_machine::state::StateMachineMessage<S>>,
    ) -> Self {
        Self { sm, initial_messages, _unused: PhantomData }
    }
}

impl<S, M> StateMachine for StandardStateMachine<S, M>
where
    S: StandardStateMachineState<M>,
    M: Send + 'static,
{
    type Result = anyhow::Result<Vec<<S::FinalResult as EncodeableOutput>::Output>>;
    type Message = S::OutputMessage;

    fn initialize(&mut self) -> anyhow::Result<EncodedYield<Self::Result, Self::Message>> {
        let messages = mem::take(&mut self.initial_messages);
        if messages.is_empty() { Ok(EncodedYield::Empty) } else { Ok(EncodedYield::Messages(messages)) }
    }

    fn proceed(
        &mut self,
        message: PartyMessage<Self::Message>,
    ) -> anyhow::Result<EncodedYield<Self::Result, Self::Message>> {
        let output = match self.sm.handle_message(message)? {
            state_machine::StateMachineOutput::Messages(m) => EncodedYield::Messages(m),
            state_machine::StateMachineOutput::Final(o) => EncodedYield::Result(o.encode()),
            state_machine::StateMachineOutput::Empty => EncodedYield::Empty,
        };
        Ok(output)
    }
}

pub(crate) trait EncodeableOutput {
    type Output: Send;

    fn encode(&self) -> anyhow::Result<Vec<Self::Output>>;
}

#[derive(thiserror::Error, Debug)]
#[error("channel dropped")]
pub(crate) struct ChannelDropped;

/// The argument to a state machine.
pub(crate) struct StateMachineArgs<I: StateMachineIo> {
    pub(crate) id: Uuid,
    pub(crate) our_party_id: PartyId,
    pub(crate) channels: Arc<dyn ClusterChannels>,
    pub(crate) timeout: Duration,
    pub(crate) name: &'static str,
    pub(crate) io: I,
    pub(crate) handles: Arc<Mutex<HashMap<Uuid, StateMachineHandle<I>>>>,
    pub(crate) cancel_token: Option<CancellationToken>,
}

/// Allows running a state machine.
pub(crate) struct StateMachineRunner<I> {
    timeout: Duration,
    our_party_id: PartyId,
    name: &'static str,
    _unused: PhantomData<I>,
}

impl<I> StateMachineRunner<I>
where
    I: StateMachineIo,
{
    pub(crate) fn start(args: StateMachineArgs<I>) -> StateMachineHandle<I> {
        let StateMachineArgs { id, our_party_id, channels, timeout, io, name, handles, cancel_token } = args;
        let (init_sender, init_receiver) = channel(SIGNAL_CHANNEL_SIZE);
        let join_handle = tokio::spawn(
            async move {
                let _guard = METRICS.active_state_machines_guard(name);
                let _timer = METRICS.state_machine_timer(name);
                let runner = Self { timeout, our_party_id, _unused: Default::default(), name };
                let run = runner.run(init_receiver, channels, io);
                match cancel_token {
                    Some(token) => {
                        if token.run_until_cancelled(run).await.is_none() {
                            warn!("Node shutting down, aborting");
                        }
                    }
                    None => run.await,
                };
                if handles.lock().await.remove(&id).is_none() {
                    warn!("Could not remove state machine handle {id}");
                }
            }
            .instrument(info_span!("stateful.state_machine", name = name, id = id.to_string())),
        );
        StateMachineHandle { init_sender, join_handle }
    }

    async fn run(self, init_receiver: Receiver<InitMessage<I>>, channels: Arc<dyn ClusterChannels>, io: I) {
        let started_at = Instant::now();
        let result = self.do_run(init_receiver, channels, &io).await;
        info!("State machine execution took {:?}", started_at.elapsed());
        io.handle_final_result(result).await;
    }

    async fn do_run(
        self,
        init_receiver: Receiver<InitMessage<I>>,
        channels: Arc<dyn ClusterChannels>,
        io: &I,
    ) -> anyhow::Result<(I::Result, I::Metadata)> {
        let parties = channels.other_parties();
        // open a channel to every party
        let mut futs = Vec::new();
        for party in &parties {
            let party_id = &party.party_id;
            info!("Opening stream to {party_id}");
            futs.push(io.open_party_stream(channels.deref(), &party.party_id));
        }
        // ensure opening the channel succeeded
        let streams = self.await_requests_with_timeout(futs).await.context("failed to establish channel to peer")?;
        let output_streams = parties.iter().map(|p| p.party_id.clone()).zip(streams).collect();

        // initialize our vm
        let (state, initialize_yield) =
            match timeout(self.timeout, Self::wait_initialization(init_receiver, output_streams, parties)).await {
                Ok(result) => result?,
                Err(_) => {
                    bail!("timed out waiting for initialization");
                }
            };
        let mut output = self.handle_vm_yield(state, initialize_yield).await?;

        // iterate until we either hit an error or we complete the execution
        loop {
            match output {
                HandleOutput::Done { output, metadata } => {
                    return Ok((output, metadata));
                }
                HandleOutput::Running(state) => {
                    let (mut state, message) = self.wait_messages(state).await?;
                    let vm_yield = state.vm.proceed(message)?;
                    output = self.handle_vm_yield(state, vm_yield).await?;
                }
            };
        }
    }

    async fn await_requests_with_timeout<F, T, E>(&self, futs: Vec<F>) -> anyhow::Result<Vec<T>>
    where
        F: Future<Output = Result<T, E>>,
        E: std::error::Error + Send + Sync + 'static,
    {
        match timeout(self.timeout, join_all(futs)).await {
            Ok(futs) => Ok(futs.into_iter().collect::<Result<_, _>>()?),
            Err(_) => bail!("timed out waiting for request to be handled"),
        }
    }

    async fn wait_messages(
        &self,
        mut state: State<I>,
    ) -> anyhow::Result<(State<I>, PartyMessage<I::StateMachineMessage>)> {
        // Note that because we actually have a channel (with buffer space) in between nodes,
        // sending should be very fast unless we're blocked because the channel is full. And the
        // channel can only be full if the receiver node is down or not pulling fast enough.
        //
        // Ultimate the _real_ timeout happens in `await_requests` above which is where we wait for
        // someone to send us a message. e.g. we could push messages to the channel for a node
        // that's unresponsive and the side effect is we will eventually timeout while waiting for
        // someone to send us a message rather than while sending the message to that node.
        match timeout(self.timeout, state.input_stream.next()).await {
            Ok(Some(msg)) => {
                METRICS.inc_messages(self.name, "rx", 1);
                Ok((state, msg))
            }
            Ok(None) => Err(anyhow!("no more input messages")),
            Err(_) => Err(anyhow!("timed out waiting for incoming messages")),
        }
    }

    async fn handle_vm_yield(
        &self,
        mut state: State<I>,
        vm_yield: EncodedYield<I::Result, I::StateMachineMessage>,
    ) -> anyhow::Result<HandleOutput<I>> {
        let mut vm_yields = vec![vm_yield];
        while !vm_yields.is_empty() {
            // process any pending yields. yields generated by this round will be queued into
            // `vm_yields` and processed on the next loop.
            let current_yields = std::mem::take(&mut vm_yields);
            for vm_yield in current_yields {
                let messages = match vm_yield {
                    EncodedYield::Result(output) => {
                        return Ok(HandleOutput::Done { output, metadata: state.metadata });
                    }
                    EncodedYield::Empty => continue,
                    EncodedYield::Messages(messages) => messages,
                };
                // accumulate all of our own messages and the ones to be sent for later use.
                let (self_messages, futs) = self.split_messages(&state, messages)?;
                info!("Need to send {} messages and have {} to be handled locally", futs.len(), self_messages.len());
                METRICS.inc_messages(self.name, "tx", futs.len() as u64);
                // send all messages and ensure they all succeeded.
                self.await_requests_with_timeout(futs).await.context("party dropped stream before completion")?;
                for message in self_messages {
                    let vm_yield = state.vm.proceed(PartyMessage::new(self.our_party_id.clone(), message))?;
                    vm_yields.push(vm_yield);
                }
            }
        }
        Ok(HandleOutput::Running(state))
    }

    fn split_messages<'a>(
        &self,
        state: &'a State<I>,
        messages: Vec<RecipientMessage<PartyId, I::StateMachineMessage>>,
    ) -> anyhow::Result<(
        Vec<I::StateMachineMessage>,
        Vec<impl Future<Output = Result<(), SendError<I::OutputMessage>>> + 'a>,
    )> {
        // accumulate all of our own messages and the ones to be sent for later use.
        let mut self_messages = Vec::new();
        let mut futs = Vec::new();
        for message in messages {
            let (recipient, message) = message.into_parts();
            let recipients = match recipient {
                Recipient::Single(party_id) => vec![party_id],
                Recipient::Multiple(parties) => parties,
            };
            let mut serialized_message: Option<Vec<u8>> = None;
            for party in recipients {
                if party == self.our_party_id {
                    self_messages.push(message.clone());
                } else {
                    let Some(channel) = state.output_streams.get(&party) else {
                        bail!("state machine requested message be sent to party outside of cluster: {party}");
                    };
                    // serialize this only once on demand
                    let encoded_message = match &serialized_message {
                        Some(message) => message.clone(),
                        None => {
                            let message = message.try_encode().context("serializing message")?;
                            serialized_message = Some(message.clone());
                            message
                        }
                    };
                    let message = I::StateMachineMessage::encoded_bytes_as_output_message(encoded_message);
                    futs.push(channel.send(message));
                }
            }
        }
        Ok((self_messages, futs))
    }

    async fn wait_initialization(
        mut init_receiver: Receiver<InitMessage<I>>,
        output_streams: HashMap<PartyId, Sender<I::OutputMessage>>,
        parties: Vec<Party>,
    ) -> anyhow::Result<(State<I>, EncodedYield<I::Result, I::StateMachineMessage>)> {
        let mut vm_state = None;
        let mut input_streams = HashMap::new();
        let peer_count = output_streams.len();
        while let Some(message) = init_receiver.recv().await {
            match message {
                InitMessage::InitStateMachine { state_machine: mut vm, metadata } => {
                    info!("Initialized state machine");
                    let vm_yield = vm.initialize()?;
                    vm_state = Some((vm, vm_yield, metadata));
                }
                InitMessage::InitParty { user_id, stream } => {
                    let Some(party) = parties.iter().find(|p| p.user_id == user_id) else {
                        warn!("User {user_id} doesn't map to any cluster parties");
                        continue;
                    };
                    let party_id = party.party_id.clone();
                    if input_streams.insert(party_id.clone(), stream).is_some() {
                        warn!("Received duplicate stream from {party_id}");
                    }
                    info!("Initialized peer stream for {party_id}");
                }
            };
            if vm_state.is_some() && input_streams.len() == peer_count {
                break;
            }
        }
        let Some((vm, vm_yield, metadata)) = vm_state else {
            error!("Init stream reached EOF before state machine was initialized");
            bail!("state machine initialization failed");
        };
        if input_streams.len() != peer_count {
            error!("Init stream reach EOF before initialization was completed");
            bail!("state machine initialization failed");
        }
        // Group all input streams into a single one that spits out a `PartyMessage`
        let input_streams = input_streams
            .into_iter()
            .map(|(party, stream)| ReceiverStream::new(stream).map(move |m| PartyMessage::new(party.clone(), m)));
        let input_stream = Box::new(select_all(input_streams));
        let state = State { vm, input_stream, output_streams, metadata };
        Ok((state, vm_yield))
    }
}

enum HandleOutput<I>
where
    I: StateMachineIo,
{
    Done { output: I::Result, metadata: I::Metadata },
    Running(State<I>),
}

struct State<I>
where
    I: StateMachineIo,
{
    vm: BoxStateMachine<I>,
    input_stream: Box<dyn Stream<Item = PartyMessage<I::StateMachineMessage>> + Send + Unpin>,
    output_streams: HashMap<PartyId, Sender<I::OutputMessage>>,
    metadata: I::Metadata,
}

struct Metrics {
    messages: MaybeMetric<Counter>,
    active_state_machines: MaybeMetric<Gauge>,
    execution_duration: MaybeMetric<Histogram<Duration>>,
}

impl Default for Metrics {
    fn default() -> Self {
        let messages = Counter::new(
            "state_machine_messages_total",
            "Number of messages tx/rx during the execution of a state machine",
            &["state_machine", "direction"],
        )
        .into();
        let active_state_machines =
            Gauge::new("active_state_machines_total", "Number of active state machines", &["state_machine"]).into();
        let execution_duration = Histogram::new(
            "state_machine_execution_duration_seconds",
            "Amount of time taken for a state machine's execution",
            &["state_machine"],
            TimingBuckets::sub_minute(),
        )
        .into();
        Self { messages, active_state_machines, execution_duration }
    }
}

impl Metrics {
    fn inc_messages(&self, state_machine: &str, direction: &str, count: u64) {
        self.messages.with_labels([("state_machine", state_machine), ("direction", direction)]).inc_by(count);
    }

    fn active_state_machines_guard(&self, state_machine: &str) -> ScopedGauge<impl SingleGaugeMetric> {
        self.active_state_machines.with_labels([("state_machine", state_machine)]).into_scoped_gauge()
    }

    fn state_machine_timer(&self, state_machine: &str) -> ScopedTimer<impl SingleHistogramMetric<Duration>> {
        self.execution_duration.with_labels([("state_machine", state_machine)]).into_timer()
    }
}
