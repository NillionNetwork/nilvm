//! The programs API.

/// The protobuf model definitions.
pub mod proto {
    pub use crate::proto::programs::v1::*;
}

/// Rust types that can be converted from/to their protobuf counterparts.
#[cfg(feature = "rust-types")]
pub mod rust {
    use crate::{payments::rust::SignedReceipt, ConvertProto, ProtoError, TransparentProto, TryIntoRust};

    /// A response to a request to store a program in the network.
    pub type StoreProgramResponse = super::proto::store::StoreProgramResponse;

    impl TransparentProto for StoreProgramResponse {}

    /// A request to store a program in the network.
    #[derive(Clone, Debug, PartialEq)]
    pub struct StoreProgramRequest {
        /// The contents of the program.
        pub program: Vec<u8>,

        /// The receipt that proves this operation was paid for.
        pub signed_receipt: SignedReceipt,
    }

    impl ConvertProto for StoreProgramRequest {
        type ProtoType = super::proto::store::StoreProgramRequest;

        fn into_proto(self) -> Self::ProtoType {
            Self::ProtoType { program: self.program, signed_receipt: Some(self.signed_receipt.into_proto()) }
        }

        fn try_from_proto(model: Self::ProtoType) -> Result<Self, crate::ProtoError> {
            let signed_receipt = model.signed_receipt.ok_or(ProtoError("'signed_receipt' not set"))?.try_into_rust()?;
            Ok(Self { program: model.program, signed_receipt })
        }
    }
}
