//! Channels for nodes to communicate with each other.

use crate::stateful::STREAM_CHANNEL_SIZE;
use async_trait::async_trait;
use basic_types::PartyId;
use futures::StreamExt;
use grpc_channel::{token::TokenAuthenticator, AuthenticatedGrpcChannel, GrpcChannelConfig, TransportChannel};
use node_api::{
    auth::rust::{PublicKey, UserId},
    compute::{proto::compute_client::ComputeClient, rust::ComputeStreamMessage},
    membership::rust::Cluster,
    preprocessing::{
        proto::{
            preprocessing_client::PreprocessingClient,
            stream::{AuxiliaryMaterialStreamMessage, PreprocessingStreamMessage},
        },
        rust::{
            CleanupUsedElementsRequest, GenerateAuxiliaryMaterialRequest, GenerateAuxiliaryMaterialResponse,
            GeneratePreprocessingRequest, GeneratePreprocessingResponse,
        },
    },
    ConvertProto, TryIntoRust,
};
use std::{collections::HashMap, future::Future, iter, time::Duration};
use tokio::{
    sync::mpsc::{channel, Receiver, Sender},
    time::sleep,
};
use tokio_stream::wrappers::ReceiverStream;
use tonic::{Request, Status};
use tracing::{error, info, instrument, warn};
use user_keypair::SigningKey;

const TOKEN_EXPIRY: Duration = Duration::from_secs(60);
const CHANNEL_TIMEOUT: Duration = Duration::from_secs(60);
const PREPROCESSING_CHANNEL_SIZE: usize = 128;
const MAX_RETRIES: usize = 15;
const RETRY_DELAY: Duration = Duration::from_secs(2);

#[derive(Clone, Debug)]
pub(crate) struct Party {
    pub(crate) party_id: PartyId,
    pub(crate) user_id: UserId,
}

#[cfg_attr(test, mockall::automock)]
#[async_trait]
pub(crate) trait ClusterChannels: Send + Sync + 'static {
    /// Get all parties that are part of this cluster, including ourselves.
    fn all_parties(&self) -> Vec<Party>;

    /// Get parties that are part of this cluster, excluding ourselves.
    fn other_parties(&self) -> Vec<Party>;

    /// Checks whether the given party is a part of this cluster.
    fn is_member(&self, user_id: &UserId) -> bool;

    /// Open a compute stream to the given party.
    async fn open_compute_stream(
        &self,
        party: &PartyId,
        initial_message: ComputeStreamMessage,
    ) -> tonic::Result<Sender<ComputeStreamMessage>>;

    /// Open a preprocessing stream to the given party.
    async fn open_preprocessing_stream(
        &self,
        party: &PartyId,
        initial_message: PreprocessingStreamMessage,
    ) -> tonic::Result<Sender<PreprocessingStreamMessage>>;

    /// Open an auxiliary material stream to the given party.
    async fn open_auxiliary_material_stream(
        &self,
        party: &PartyId,
        initial_message: AuxiliaryMaterialStreamMessage,
    ) -> tonic::Result<Sender<AuxiliaryMaterialStreamMessage>>;

    /// Tell the given party to start generating preprocessing.
    async fn generate_preprocessing(
        &self,
        party: PartyId,
        request: GeneratePreprocessingRequest,
    ) -> tonic::Result<Receiver<tonic::Result<GeneratePreprocessingResponse>>>;

    /// Tell the given party to start generating auxiliary material.
    async fn generate_auxiliary_material(
        &self,
        party: PartyId,
        request: GenerateAuxiliaryMaterialRequest,
    ) -> tonic::Result<Receiver<tonic::Result<GenerateAuxiliaryMaterialResponse>>>;

    /// Tell the given party to delete used preprocessing chunks.
    async fn cleanup_used_elements(&self, party: PartyId, request: CleanupUsedElementsRequest) -> tonic::Result<()>;
}

pub(crate) struct DefaultClusterChannels {
    channels: HashMap<PartyId, AuthenticatedGrpcChannel>,
    parties: Vec<Party>,
    our_party: Party,
}

impl DefaultClusterChannels {
    pub(crate) fn new(key: &SigningKey, cluster: &Cluster, ca_cert: Option<Vec<u8>>) -> anyhow::Result<Self> {
        let mut channels = HashMap::new();
        let mut parties = Vec::new();
        for member in &cluster.members {
            let party = match member.public_keys.authentication {
                PublicKey::Ed25519(bytes) => Self::party_from_ed25519(&bytes)?,
                PublicKey::Secp256k1(bytes) => Self::party_from_secp256k1(&bytes)?,
            };
            info!(
                "Setting up channel to member {} (user {}) @ {}",
                party.party_id, party.user_id, member.grpc_endpoint
            );
            let authenticator = TokenAuthenticator::new(key.clone(), member.identity.clone(), TOKEN_EXPIRY);
            let mut builder = GrpcChannelConfig::new(member.grpc_endpoint.clone())
                .authentication(authenticator)
                .timeout(CHANNEL_TIMEOUT);
            if let Some(cert) = ca_cert.as_ref() {
                // This should be made configurable
                builder = builder.ca_certificate(cert).domain("nillion.local");
            }
            let channel = builder.build()?;
            channels.insert(party.party_id.clone(), channel);
            parties.push(party);
        }
        let our_party = Self::party_from_ed25519(&key.public_key().as_bytes())?;
        Ok(Self { channels, parties, our_party })
    }

    fn party_from_ed25519(public_key: &[u8]) -> anyhow::Result<Party> {
        let user_id = UserId::from_bytes(public_key);
        let party_id = PartyId::from(user_id.as_ref());
        Ok(Party { party_id, user_id })
    }

    fn party_from_secp256k1(public_key: &[u8]) -> anyhow::Result<Party> {
        let user_id = UserId::from_bytes(public_key);
        let party_id = PartyId::from(user_id.as_ref());
        Ok(Party { party_id, user_id })
    }

    fn party_channel(&self, party: &PartyId) -> tonic::Result<&AuthenticatedGrpcChannel> {
        self.channels.get(party).ok_or_else(|| Status::internal(format!("channel for peer {party} not found")))
    }

    async fn try_open_stream<M, H, O>(&self, initial_message: M, mut callback: H) -> tonic::Result<Sender<M>>
    where
        M: Clone + Send + 'static,
        H: FnMut(ReceiverStream<M>) -> O,
        O: Future<Output = tonic::Result<tonic::Response<()>>>,
    {
        let mut retries = iter::repeat(RETRY_DELAY).take(MAX_RETRIES);
        loop {
            let (sender, receiver) = channel(STREAM_CHANNEL_SIZE);
            sender.send(initial_message.clone()).await.map_err(|_| Status::internal("channel dropped"))?;

            let messages = ReceiverStream::new(receiver);
            match callback(messages).await {
                Ok(_) => return Ok(sender),
                Err(e) => match retries.next() {
                    Some(delay) => {
                        warn!("Request failed, retrying in {RETRY_DELAY:?}: {e}");
                        sleep(delay).await;
                    }
                    None => {
                        error!("Failed to invoke request and we're out of retries: {e}");
                        return Err(e);
                    }
                },
            }
        }
    }
}

#[async_trait]
impl ClusterChannels for DefaultClusterChannels {
    fn all_parties(&self) -> Vec<Party> {
        self.parties.clone()
    }

    fn other_parties(&self) -> Vec<Party> {
        self.parties.iter().filter(|p| p.party_id != self.our_party.party_id).cloned().collect()
    }

    fn is_member(&self, user_id: &UserId) -> bool {
        self.parties.iter().any(|p| &p.user_id == user_id)
    }

    async fn open_compute_stream(
        &self,
        party: &PartyId,
        initial_message: ComputeStreamMessage,
    ) -> tonic::Result<Sender<ComputeStreamMessage>> {
        let channel = self.party_channel(party)?;
        let client = ComputeClient::new(channel.clone().into_channel());
        self.try_open_stream(initial_message, move |stream| {
            let mut client = client.clone();
            async move { client.stream_compute(stream).await }
        })
        .await
    }

    async fn open_preprocessing_stream(
        &self,
        party: &PartyId,
        initial_message: PreprocessingStreamMessage,
    ) -> tonic::Result<Sender<PreprocessingStreamMessage>> {
        let channel = self.party_channel(party)?;
        let client = PreprocessingClient::new(channel.clone().into_channel());
        self.try_open_stream(initial_message, move |stream| {
            let mut client = client.clone();
            async move { client.stream_preprocessing(stream).await }
        })
        .await
    }

    async fn open_auxiliary_material_stream(
        &self,
        party: &PartyId,
        initial_message: AuxiliaryMaterialStreamMessage,
    ) -> tonic::Result<Sender<AuxiliaryMaterialStreamMessage>> {
        let channel = self.party_channel(party)?;
        let client = PreprocessingClient::new(channel.clone().into_channel());
        self.try_open_stream(initial_message, move |stream| {
            let mut client = client.clone();
            async move { client.stream_auxiliary_material(stream).await }
        })
        .await
    }

    #[instrument("cluster.generate_preprocessing", skip_all, fields(party = party.to_string()))]
    async fn generate_preprocessing(
        &self,
        party: PartyId,
        request: GeneratePreprocessingRequest,
    ) -> tonic::Result<Receiver<tonic::Result<GeneratePreprocessingResponse>>> {
        let Some(channel) = self.channels.get(&party) else {
            return Err(Status::internal(format!("channel for peer {party} not found")));
        };
        let mut client = PreprocessingClient::new(channel.clone().into_channel());
        let request = Request::new(request.into_proto());
        let mut stream = client.generate_preprocessing(request).await?.into_inner();

        // create an intermediate stream and forward between them. this allows us to detach the
        // function signature from tonic specific stream types
        let (tx, rx) = tokio::sync::mpsc::channel(PREPROCESSING_CHANNEL_SIZE);
        tokio::spawn(async move {
            while let Some(message) = stream.next().await {
                let message = message.and_then(|message| {
                    message
                        .try_into_rust()
                        .map_err(|e| Status::invalid_argument(format!("invalid generate preprocessing response: {e}")))
                });
                if tx.send(message).await.is_err() {
                    warn!("Preprocessing execution receiver dropped, aborting");
                    break;
                }
            }
        });
        Ok(rx)
    }

    #[instrument("cluster.generate_auxiliary_material", skip_all, fields(party = party.to_string()))]
    async fn generate_auxiliary_material(
        &self,
        party: PartyId,
        request: GenerateAuxiliaryMaterialRequest,
    ) -> tonic::Result<Receiver<tonic::Result<GenerateAuxiliaryMaterialResponse>>> {
        let Some(channel) = self.channels.get(&party) else {
            return Err(Status::internal(format!("channel for peer {party} not found")));
        };
        let mut client = PreprocessingClient::new(channel.clone().into_channel());
        let request = Request::new(request.into_proto());
        let mut stream = client.generate_auxiliary_material(request).await?.into_inner();

        let (tx, rx) = tokio::sync::mpsc::channel(PREPROCESSING_CHANNEL_SIZE);
        tokio::spawn(async move {
            while let Some(message) = stream.next().await {
                let message = message.and_then(|message| {
                    message.try_into_rust().map_err(|e| {
                        Status::invalid_argument(format!("invalid generate auxiliary material response: {e}"))
                    })
                });
                if tx.send(message).await.is_err() {
                    warn!("Auxiliary material execution receiver dropped, aborting");
                    break;
                }
            }
        });
        Ok(rx)
    }

    async fn cleanup_used_elements(&self, party: PartyId, request: CleanupUsedElementsRequest) -> tonic::Result<()> {
        let Some(channel) = self.channels.get(&party) else {
            return Err(Status::internal(format!("channel for peer {party} not found")));
        };
        let mut client = PreprocessingClient::new(channel.clone().into_channel());
        let request = Request::new(request.into_proto());
        client.cleanup_used_elements(request).await?;
        Ok(())
    }
}
