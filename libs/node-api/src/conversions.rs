//! Conversion traits.

use chrono::{DateTime, Utc};
use prost::Message;
use std::{collections::HashSet, hash::Hash};
use uuid::Uuid;

/// A trait that allows converting a trait from/into protobuf.
pub trait ConvertProto: Sized {
    /// The protobuf type that represents this type.
    type ProtoType;

    /// Convert this type into protobuf.
    fn into_proto(self) -> Self::ProtoType;

    /// Try to construct an instance from a protobuf type.
    fn try_from_proto(model: Self::ProtoType) -> Result<Self, ProtoError>;

    fn try_decode(bytes: &[u8]) -> Result<Self, ProtoError>
    where
        Self::ProtoType: Message + Default,
    {
        let model = Self::ProtoType::decode(bytes).map_err(|_| ProtoError("protobuf decoding failed"))?;
        model.try_into_rust()
    }
}

impl<T: ConvertProto> ConvertProto for Vec<T> {
    type ProtoType = Vec<T::ProtoType>;

    fn into_proto(self) -> Self::ProtoType {
        self.into_iter().map(T::into_proto).collect()
    }

    fn try_from_proto(model: Self::ProtoType) -> Result<Self, ProtoError> {
        model.into_iter().map(T::try_from_proto).collect()
    }
}

impl<T> ConvertProto for HashSet<T>
where
    T: Eq + Hash + PartialEq + ConvertProto,
{
    type ProtoType = Vec<T::ProtoType>;

    fn into_proto(self) -> Self::ProtoType {
        self.into_iter().map(T::into_proto).collect()
    }

    fn try_from_proto(model: Self::ProtoType) -> Result<Self, ProtoError> {
        model.into_iter().map(T::try_from_proto).collect()
    }
}

/// Try to convert a protobuf model into a rust type.
pub trait TryIntoRust<T> {
    /// Try to convert this protobuf model into a rust type.
    fn try_into_rust(self) -> Result<T, ProtoError>;
}

impl<T, U> TryIntoRust<T> for U
where
    T: ConvertProto<ProtoType = U>,
{
    fn try_into_rust(self) -> Result<T, ProtoError> {
        T::try_from_proto(self)
    }
}

#[derive(Debug, thiserror::Error)]
#[error("protobuf parsing error: {0}")]
pub struct ProtoError(pub &'static str);

impl From<ProtoError> for tonic::Status {
    fn from(error: ProtoError) -> Self {
        Self::invalid_argument(error.to_string())
    }
}

impl ConvertProto for DateTime<Utc> {
    type ProtoType = prost_types::Timestamp;

    fn into_proto(self) -> Self::ProtoType {
        Self::ProtoType { seconds: self.timestamp(), nanos: self.timestamp_subsec_nanos() as i32 }
    }

    fn try_from_proto(model: Self::ProtoType) -> Result<Self, ProtoError> {
        let nanos = model.nanos.try_into().map_err(|_| ProtoError("'nanos' is negative"))?;
        DateTime::from_timestamp(model.seconds, nanos).ok_or(ProtoError("invalid timestamp"))
    }
}

impl ConvertProto for Uuid {
    type ProtoType = String;

    fn into_proto(self) -> Self::ProtoType {
        self.to_string()
    }

    fn try_from_proto(model: Self::ProtoType) -> Result<Self, ProtoError> {
        model.parse().map_err(|_| ProtoError("invalid uuid"))
    }
}
/// A marker trait that indicates a type's protobuf model is the same as the rust one.
///
/// This allows always using `ConvertProto` for a type without having to know if the rust type is
/// the same as the protobuf one.
pub trait TransparentProto {}

impl<T: TransparentProto> ConvertProto for T {
    type ProtoType = T;

    fn into_proto(self) -> Self::ProtoType {
        self
    }

    fn try_from_proto(model: Self::ProtoType) -> Result<Self, ProtoError> {
        Ok(model)
    }
}
