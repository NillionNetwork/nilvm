//! The preprocessing API.

/// The protobuf model definitions.
pub mod proto {
    pub use crate::proto::preprocessing::v1::*;
}

/// Rust types that can be converted from/to their protobuf counterparts.
#[cfg(feature = "rust-types")]
pub mod rust {
    use crate::{ConvertProto, ProtoError, TransparentProto};

    /// A preprocessing element.
    pub type PreprocessingElement = super::proto::element::PreprocessingElement;

    /// An auxiliary material.
    pub type AuxiliaryMaterial = super::proto::material::AuxiliaryMaterial;

    /// The status of a preprocessing protocol execution.
    pub type PreprocessingProtocolStatus = super::proto::generate::PreprocessingProtocolStatus;

    impl TransparentProto for PreprocessingElement {}
    impl TransparentProto for AuxiliaryMaterial {}
    impl TransparentProto for PreprocessingProtocolStatus {}

    /// A request to generate preprocessing material.
    #[derive(Clone, Debug, PartialEq)]
    pub struct GeneratePreprocessingRequest {
        // An identifier for this generation instance.
        pub generation_id: Vec<u8>,

        // The batch id that is being generated.
        //
        // This is a sequential number per preprocessing element.
        pub batch_id: u64,

        // The number of elements being generated.
        pub batch_size: u32,

        // The preprocessing element being generated.
        pub element: PreprocessingElement,
    }

    impl ConvertProto for GeneratePreprocessingRequest {
        type ProtoType = super::proto::generate::GeneratePreprocessingRequest;

        fn into_proto(self) -> Self::ProtoType {
            let Self { generation_id, batch_id, batch_size, element } = self;
            Self::ProtoType { generation_id, batch_id, batch_size, element: element.into() }
        }

        fn try_from_proto(model: Self::ProtoType) -> Result<Self, crate::ProtoError> {
            let Self::ProtoType { generation_id, batch_id, batch_size, element } = model;
            let element = element.try_into().map_err(|_| ProtoError("invalid 'element' field"))?;
            Ok(Self { generation_id, batch_id, batch_size, element })
        }
    }

    /// A request to generate auxiliary material.
    #[derive(Clone, Debug, PartialEq)]
    pub struct GenerateAuxiliaryMaterialRequest {
        // An identifier for this generation instance.
        pub generation_id: Vec<u8>,

        // The material being generated.
        pub material: AuxiliaryMaterial,

        /// The version of material to be generated.
        pub version: u32,
    }

    impl ConvertProto for GenerateAuxiliaryMaterialRequest {
        type ProtoType = super::proto::generate::GenerateAuxiliaryMaterialRequest;

        fn into_proto(self) -> Self::ProtoType {
            let Self { generation_id, material, version } = self;
            Self::ProtoType { generation_id, material: material.into(), version }
        }

        fn try_from_proto(model: Self::ProtoType) -> Result<Self, crate::ProtoError> {
            let Self::ProtoType { generation_id, material, version } = model;
            let material = material.try_into().map_err(|_| ProtoError("invalid 'material' field"))?;
            Ok(Self { generation_id, material, version })
        }
    }

    /// The response to a request to generate preprocessing material.
    #[derive(Clone, Debug, PartialEq)]
    pub struct GeneratePreprocessingResponse {
        /// The status of the preprocessing protocol.
        pub status: PreprocessingProtocolStatus,
    }

    impl ConvertProto for GeneratePreprocessingResponse {
        type ProtoType = super::proto::generate::GeneratePreprocessingResponse;

        fn into_proto(self) -> Self::ProtoType {
            let Self { status } = self;
            Self::ProtoType { status: status.into() }
        }

        fn try_from_proto(model: Self::ProtoType) -> Result<Self, ProtoError> {
            let Self::ProtoType { status } = model;
            let status = status.try_into().map_err(|_| ProtoError("invalid 'status' field"))?;
            Ok(Self { status })
        }
    }

    /// The response to a request to generate auxiliary material.
    #[derive(Clone, Debug, PartialEq)]
    pub struct GenerateAuxiliaryMaterialResponse {
        /// The status of the generation protocol.
        pub status: PreprocessingProtocolStatus,
    }

    impl ConvertProto for GenerateAuxiliaryMaterialResponse {
        type ProtoType = super::proto::generate::GenerateAuxiliaryMaterialResponse;

        fn into_proto(self) -> Self::ProtoType {
            let Self { status } = self;
            Self::ProtoType { status: status.into() }
        }

        fn try_from_proto(model: Self::ProtoType) -> Result<Self, ProtoError> {
            let Self::ProtoType { status } = model;
            let status = status.try_into().map_err(|_| ProtoError("invalid 'status' field"))?;
            Ok(Self { status })
        }
    }

    /// A preprocessing message exchanged during generation.
    #[derive(Clone, Debug, PartialEq)]
    pub struct PreprocessingStreamMessage {
        /// The generation id.
        pub generation_id: Vec<u8>,

        /// The preprocessing element being generated.
        pub element: PreprocessingElement,

        /// The state machine message encoded in bincode.
        pub bincode_message: Vec<u8>,
    }

    impl ConvertProto for PreprocessingStreamMessage {
        type ProtoType = super::proto::stream::PreprocessingStreamMessage;

        fn into_proto(self) -> Self::ProtoType {
            let Self { generation_id, element, bincode_message } = self;
            Self::ProtoType { generation_id, element: element.into(), bincode_message }
        }

        fn try_from_proto(model: Self::ProtoType) -> Result<Self, ProtoError> {
            let Self::ProtoType { generation_id, element, bincode_message } = model;
            let element = element.try_into().map_err(|_| ProtoError("invalid 'element' field"))?;
            Ok(Self { generation_id, element, bincode_message })
        }
    }

    /// An auxiliary material message exchanged during generation.
    #[derive(Clone, Debug, PartialEq)]
    pub struct AuxiliaryMaterialStreamMessage {
        /// The generation id.
        pub generation_id: Vec<u8>,

        /// The material being generated.
        pub material: AuxiliaryMaterial,

        /// The state machine message encoded in bincode.
        pub bincode_message: Vec<u8>,
    }

    impl ConvertProto for AuxiliaryMaterialStreamMessage {
        type ProtoType = super::proto::stream::AuxiliaryMaterialStreamMessage;

        fn into_proto(self) -> Self::ProtoType {
            let Self { generation_id, material, bincode_message } = self;
            Self::ProtoType { generation_id, material: material.into(), bincode_message }
        }

        fn try_from_proto(model: Self::ProtoType) -> Result<Self, ProtoError> {
            let Self::ProtoType { generation_id, material, bincode_message } = model;
            let material = material.try_into().map_err(|_| ProtoError("invalid 'material' field"))?;
            Ok(Self { generation_id, material, bincode_message })
        }
    }

    /// A request to cleanup old preprocessing chunks.
    #[derive(Clone, Debug, PartialEq)]
    pub struct CleanupUsedElementsRequest {
        /// The element to be cleanedup.
        pub element: PreprocessingElement,

        /// The first chunk to be deleted.
        pub start_chunk: u64,

        /// The one-past-the-end chunk index to be deleted.
        pub end_chunk: u64,
    }

    impl ConvertProto for CleanupUsedElementsRequest {
        type ProtoType = super::proto::cleanup::CleanupUsedElementsRequest;

        fn into_proto(self) -> Self::ProtoType {
            let Self { element, start_chunk, end_chunk } = self;
            let element = element.into();
            Self::ProtoType { element, start_chunk, end_chunk }
        }

        fn try_from_proto(model: Self::ProtoType) -> Result<Self, ProtoError> {
            let Self::ProtoType { element, start_chunk, end_chunk } = model;
            let element = element.try_into().map_err(|_| ProtoError("invalid 'element' field"))?;
            Ok(Self { element, start_chunk, end_chunk })
        }
    }
}
