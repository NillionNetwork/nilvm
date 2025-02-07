use crate::storage::repositories::blob::{BinarySerde, BinarySerdeError};
use chrono::{DateTime, Utc};
use node_api::{
    membership::rust::Prime, permissions::rust::Permissions, values::proto::value::NamedValue, ConvertProto,
    TryIntoRust,
};
use prost::Message;

mod proto {
    tonic::include_proto!("node.values.v1.values");
}

// Don't allow serialized payloads to be larger than 16MB
const MAX_SERIALIZED_SIZE: usize = 1024 * 1024 * 16;

/// User Values stored in storage
#[derive(Clone, Debug, PartialEq)]
pub struct UserValuesRecord {
    /// Values stored by the user
    pub values: Vec<NamedValue>,

    /// Permissions
    pub permissions: Permissions,

    /// The expiration time.
    pub expires_at: DateTime<Utc>,

    /// The prime number used.
    pub prime: Prime,
}

impl BinarySerde for UserValuesRecord {
    fn serialize(self) -> Result<Vec<u8>, BinarySerdeError> {
        let model = proto::UserValues {
            values: self.values,
            permissions: Some(self.permissions.into_proto()),
            expires_at: Some(self.expires_at.into_proto()),
            prime: self.prime.into_proto(),
        };
        let bytes = model.encode_to_vec();
        let length = bytes.len();
        if length > MAX_SERIALIZED_SIZE {
            Err(BinarySerdeError(format!("serialized size ({length}) is larger than allowed ({MAX_SERIALIZED_SIZE})",)))
        } else {
            Ok(bytes)
        }
    }

    fn deserialize(bytes: &[u8]) -> Result<Self, BinarySerdeError> {
        let proto::UserValues { values, permissions, expires_at, prime } =
            proto::UserValues::decode(bytes).map_err(|e| BinarySerdeError(format!("invalid protobuf model: {e}")))?;
        let prime: Prime = prime.try_into_rust().map_err(|_| BinarySerdeError("invalid prime".into()))?;
        let permissions = permissions
            .ok_or_else(|| BinarySerdeError("'permissions' not set".into()))?
            .try_into_rust()
            .map_err(|e| BinarySerdeError(e.to_string()))?;
        let expires_at = expires_at
            .ok_or_else(|| BinarySerdeError("'expires_at' not set".into()))?
            .try_into_rust()
            .map_err(|e| BinarySerdeError(e.to_string()))?;
        Ok(UserValuesRecord { values, permissions, expires_at, prime })
    }
}
