use crate::storage::repositories::blob::{BinarySerde, BinarySerdeError};
use node_api::{auth::rust::UserId, values::rust::NamedValue, ConvertProto, ProtoError};
use prost::Message;
use std::collections::HashMap;

mod proto {
    tonic::include_proto!("node.compute.v1.result");
}

#[derive(Clone)]
pub(crate) enum ComputeResult {
    // For each output party, the output name/values.
    Success { values: HashMap<UserId, Vec<NamedValue>> },
    Failure { error: String },
}

impl BinarySerde for ComputeResult {
    fn serialize(self) -> Result<Vec<u8>, BinarySerdeError> {
        let model = match self {
            Self::Success { values } => {
                let outputs = values
                    .into_iter()
                    .map(|(user, values)| proto::UserOutputs { user: Some(user.into_proto()), values })
                    .collect();
                proto::ComputeResult {
                    result: Some(proto::compute_result::Result::Success(proto::SuccessfulComputeResult { outputs })),
                }
            }
            Self::Failure { error } => {
                proto::ComputeResult { result: Some(proto::compute_result::Result::Error(error.clone())) }
            }
        };
        let bytes = model.encode_to_vec();
        Ok(bytes)
    }

    fn deserialize(bytes: &[u8]) -> Result<Self, BinarySerdeError> {
        let proto::ComputeResult { result } = proto::ComputeResult::decode(bytes)
            .map_err(|e| BinarySerdeError(format!("invalid protobuf model: {e}")))?;
        let Some(result) = result else {
            return Err(BinarySerdeError("no result".into()));
        };
        let result = match result {
            proto::compute_result::Result::Success(result) => {
                let values = result
                    .outputs
                    .into_iter()
                    .map(|output| {
                        output
                            .user
                            .ok_or(ProtoError("'user' not set "))
                            .and_then(UserId::try_from_proto)
                            .map(|user| (user, output.values))
                    })
                    .collect::<Result<_, _>>()
                    .map_err(|e| BinarySerdeError(format!("invalid compute result: {e}")))?;
                ComputeResult::Success { values }
            }
            proto::compute_result::Result::Error(error) => ComputeResult::Failure { error },
        };
        Ok(result)
    }
}
