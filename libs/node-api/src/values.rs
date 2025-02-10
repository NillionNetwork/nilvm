//! The values API.

/// The protobuf model definitions.
pub mod proto {
    pub use crate::proto::values::v1::*;
}

/// Rust types that can be converted from/to their protobuf counterparts.
#[cfg(feature = "rust-types")]
pub mod rust {
    use crate::{
        payments::rust::SignedReceipt, permissions::rust::Permissions, ConvertProto, ProtoError, TransparentProto,
        TryIntoRust,
    };

    /// A request to delete a previously stored set of values.
    pub type DeleteValuesRequest = super::proto::delete::DeleteValuesRequest;

    /// A response to a request to retrieve values from the network.
    pub type RetrieveValuesResponse = super::proto::retrieve::RetrieveValuesResponse;

    /// A response to a request to store values in the network.
    pub type StoreValuesResponse = super::proto::store::StoreValuesResponse;

    /// A named value.
    pub type NamedValue = super::proto::value::NamedValue;

    impl TransparentProto for DeleteValuesRequest {}
    impl TransparentProto for RetrieveValuesResponse {}
    impl TransparentProto for StoreValuesResponse {}
    impl TransparentProto for NamedValue {}

    /// A request to retrieve a set of values from the network.
    #[derive(Clone, Debug, PartialEq)]
    pub struct RetrieveValuesRequest {
        /// The receipt that proves this operation was paid for.
        pub signed_receipt: SignedReceipt,
    }

    impl ConvertProto for RetrieveValuesRequest {
        type ProtoType = super::proto::retrieve::RetrieveValuesRequest;

        fn into_proto(self) -> Self::ProtoType {
            Self::ProtoType { signed_receipt: Some(self.signed_receipt.into_proto()) }
        }

        fn try_from_proto(model: Self::ProtoType) -> Result<Self, crate::ProtoError> {
            let signed_receipt = model.signed_receipt.ok_or(ProtoError("'signed_receipt' not set"))?.try_into_rust()?;
            Ok(Self { signed_receipt })
        }
    }

    /// A request to store a set of values in the network.
    #[derive(Clone, Debug, PartialEq)]
    pub struct StoreValuesRequest {
        /// The values to be stored.
        pub values: Vec<NamedValue>,

        /// The permissions to use for these secrets.
        pub permissions: Option<Permissions>,

        /// The receipt that proves this operation was paid for.
        pub signed_receipt: SignedReceipt,

        /// The identifier to use for this operation.
        pub update_identifier: Option<Vec<u8>>,
    }

    impl ConvertProto for StoreValuesRequest {
        type ProtoType = super::proto::store::StoreValuesRequest;

        fn into_proto(self) -> Self::ProtoType {
            Self::ProtoType {
                values: self.values,
                signed_receipt: Some(self.signed_receipt.into_proto()),
                permissions: self.permissions.map(ConvertProto::into_proto),
                update_identifier: self.update_identifier.unwrap_or_default(),
            }
        }

        fn try_from_proto(model: Self::ProtoType) -> Result<Self, ProtoError> {
            let permissions = model.permissions.map(|p| p.try_into_rust()).transpose()?;
            let signed_receipt = model.signed_receipt.ok_or(ProtoError("'signed_receipt' not set"))?.try_into_rust()?;
            let update_identifier =
                if model.update_identifier.is_empty() { None } else { Some(model.update_identifier) };
            Ok(Self { values: model.values, signed_receipt, permissions, update_identifier })
        }
    }
}
