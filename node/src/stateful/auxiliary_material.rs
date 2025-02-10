use super::sm::{EncodeableOutput, StandardStateMachineState, StateMachineIo, StateMachineMessage};
use crate::{channels::ClusterChannels, services::auxiliary_material::AuxiliaryMaterialService};
use anyhow::{anyhow, Context};
use async_trait::async_trait;
use basic_types::PartyId;
use encoding::codec::MessageCodec;
use node_api::{
    preprocessing::{
        proto::stream::AuxiliaryMaterialStreamMessage,
        rust::{AuxiliaryMaterial, GenerateAuxiliaryMaterialResponse, PreprocessingProtocolStatus},
    },
    ConvertProto,
};
use protocols::threshold_ecdsa::auxiliary_information::{
    output::{EcdsaAuxInfo, EcdsaAuxInfoOutput},
    EcdsaAuxInfoStateMessage,
};
use std::{marker::PhantomData, sync::Arc};
use tokio::sync::mpsc::Sender;
use tracing::{error, info, warn};
use uuid::Uuid;

pub(crate) struct AuxiliaryMaterialStateMachineIo<S>
where
    S: StandardStateMachineState<AuxiliaryMaterialStreamMessage>,
{
    pub(crate) generation_id: Uuid,
    blob_service: Arc<dyn AuxiliaryMaterialService<<S::FinalResult as EncodeableOutput>::Output>>,
    material: AuxiliaryMaterial,
    _unused: PhantomData<S>,
}

impl<S: StandardStateMachineState<AuxiliaryMaterialStreamMessage>> AuxiliaryMaterialStateMachineIo<S> {
    pub(crate) fn new(
        generation_id: Uuid,
        blob_service: Arc<dyn AuxiliaryMaterialService<<S::FinalResult as EncodeableOutput>::Output>>,
        material: AuxiliaryMaterial,
    ) -> Self {
        Self { generation_id, blob_service, material, _unused: Default::default() }
    }
}

#[async_trait]
impl<S> StateMachineIo for AuxiliaryMaterialStateMachineIo<S>
where
    S: StandardStateMachineState<AuxiliaryMaterialStreamMessage>,
{
    type StateMachineMessage = S::OutputMessage;
    type OutputMessage = AuxiliaryMaterialStreamMessage;
    type Result = anyhow::Result<Vec<<S::FinalResult as EncodeableOutput>::Output>>;
    type Metadata = AuxiliaryMaterialMetadata;

    async fn open_party_stream(
        &self,
        channels: &dyn ClusterChannels,
        party_id: &PartyId,
    ) -> tonic::Result<Sender<Self::OutputMessage>> {
        let initial_message = node_api::preprocessing::rust::AuxiliaryMaterialStreamMessage {
            generation_id: self.generation_id.as_bytes().to_vec(),
            material: self.material,
            bincode_message: vec![],
        }
        .into_proto();
        channels.open_auxiliary_material_stream(party_id, initial_message).await
    }

    async fn handle_final_result(&self, result: anyhow::Result<(Self::Result, Self::Metadata)>) {
        // flatten the inner result
        let result = result.and_then(|(r, m)| r.map(|r| (r, m)));
        match result {
            Ok((output, metadata)) => {
                if output.len() > 1 {
                    error!("Generated more than one output");
                    return;
                }
                let Some(output) = output.into_iter().next() else {
                    error!("No outputs generated");
                    return;
                };
                let status = if let Err(e) = self.blob_service.upsert(metadata.version, output).await {
                    error!("Failed to store result: {e}");
                    PreprocessingProtocolStatus::FinishedFailure
                } else {
                    info!("Result persisted successfully");
                    PreprocessingProtocolStatus::FinishedSuccess
                };
                let response = GenerateAuxiliaryMaterialResponse { status }.into_proto();
                if metadata.response_channel.send(Ok(response)).await.is_err() {
                    warn!("Leader channel dropped before we could send response");
                }
            }
            Err(e) => {
                warn!("Auxiliary material generation execution failed: {e}");
            }
        };
    }
}

pub(crate) struct AuxiliaryMaterialMetadata {
    pub(crate) response_channel:
        Sender<tonic::Result<node_api::preprocessing::proto::generate::GenerateAuxiliaryMaterialResponse>>,
    pub(crate) version: u32,
}

impl EncodeableOutput for EcdsaAuxInfoOutput<EcdsaAuxInfo> {
    type Output = EcdsaAuxInfo;

    fn encode(&self) -> anyhow::Result<Vec<Self::Output>> {
        match self {
            EcdsaAuxInfoOutput::Success { element } => Ok(vec![element.clone()]),
            EcdsaAuxInfoOutput::Abort { reason } => Err(anyhow!("protocol failed: {reason}")),
        }
    }
}

impl StateMachineMessage<AuxiliaryMaterialStreamMessage> for EcdsaAuxInfoStateMessage {
    fn try_encode(&self) -> anyhow::Result<Vec<u8>> {
        MessageCodec.encode(self).context("serializing message")
    }

    fn try_decode(bytes: &[u8]) -> anyhow::Result<Self> {
        MessageCodec.decode(bytes).context("deserializing message")
    }

    fn encoded_bytes_as_output_message(message: Vec<u8>) -> AuxiliaryMaterialStreamMessage {
        // generation id and material are only necessary on the first message
        node_api::preprocessing::rust::AuxiliaryMaterialStreamMessage {
            generation_id: vec![],
            material: AuxiliaryMaterial::Cggmp21AuxiliaryInfo,
            bincode_message: message,
        }
        .into_proto()
    }
}
