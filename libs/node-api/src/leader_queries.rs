//! The leader queries API.

/// The protobuf model definitions.
pub mod proto {
    pub use crate::proto::leader_queries::v1::*;
}

/// Rust types that can be converted from/to their protobuf counterparts.
#[cfg(feature = "rust-types")]
pub mod rust {
    use super::proto::pool_status::PreprocessingOffsets;
    use crate::{payments::rust::SignedReceipt, preprocessing::rust::PreprocessingElement, ConvertProto, ProtoError};
    use std::{collections::BTreeMap, ops::Range};

    /// A request to get the preprocessing pool status.
    #[derive(Clone, Debug, PartialEq)]
    pub struct PoolStatusRequest {
        /// The receipt for the operation payment.
        pub signed_receipt: SignedReceipt,
    }

    impl ConvertProto for PoolStatusRequest {
        type ProtoType = super::proto::pool_status::PoolStatusRequest;

        fn into_proto(self) -> Self::ProtoType {
            Self::ProtoType { signed_receipt: Some(self.signed_receipt) }
        }

        fn try_from_proto(model: Self::ProtoType) -> Result<Self, ProtoError> {
            let signed_receipt = model.signed_receipt.ok_or(ProtoError("'receipt' not set"))?;
            Ok(Self { signed_receipt })
        }
    }

    /// A pool status response
    #[derive(Clone, Debug, PartialEq)]
    pub struct PoolStatusResponse {
        pub offsets: BTreeMap<PreprocessingElement, Range<u64>>,
        pub auxiliary_material_available: bool,
        pub preprocessing_active: bool,
    }

    impl ConvertProto for PoolStatusResponse {
        type ProtoType = super::proto::pool_status::PoolStatusResponse;

        fn into_proto(self) -> Self::ProtoType {
            let offsets = self
                .offsets
                .into_iter()
                .map(|(element, range)| super::proto::pool_status::PreprocessingOffsets {
                    element: element.into(),
                    start: range.start,
                    end: range.end,
                })
                .collect();
            Self::ProtoType {
                offsets,
                auxiliary_material_available: self.auxiliary_material_available,
                preprocessing_active: self.preprocessing_active,
            }
        }

        fn try_from_proto(model: Self::ProtoType) -> Result<Self, ProtoError> {
            let mut offsets = BTreeMap::new();
            for element_offsets in model.offsets {
                let PreprocessingOffsets { element, start, end } = element_offsets;
                let element =
                    PreprocessingElement::try_from(element).map_err(|_| ProtoError("invalid preprocessing element"))?;
                offsets.insert(element, start..end);
            }
            Ok(Self {
                offsets,
                auxiliary_material_available: model.auxiliary_material_available,
                preprocessing_active: model.preprocessing_active,
            })
        }
    }
}
