//! The payments API.

/// The protobuf model definitions.
pub mod proto {
    pub use crate::proto::payments::v1::*;
}

/// Rust types that can be converted from/to their protobuf counterparts.
#[cfg(feature = "rust-types")]
pub mod rust {
    use super::proto;
    use crate::{
        auth::rust::UserId,
        preprocessing::rust::{AuxiliaryMaterial, PreprocessingElement},
        ConvertProto, ProtoError, TransparentProto, TryIntoRust,
    };
    use chrono::{DateTime, Utc};
    use std::collections::HashMap;

    /// The fees associated with a price quote.
    pub type QuoteFees = proto::quote::QuoteFees;

    /// A signed quote.
    pub type SignedQuote = proto::quote::SignedQuote;

    /// A signed payment receipt.
    pub type SignedReceipt = proto::receipt::SignedReceipt;

    /// A retrieve values operation.
    pub type RetrieveValues = proto::quote::RetrieveValues;

    /// A store values operation.
    pub type StoreValues = proto::quote::StoreValues;

    /// A retrieve permissions operation.
    pub type RetrievePermissions = proto::quote::RetrievePermissions;

    /// An overwrite permissions operation.
    pub type OverwritePermissions = proto::quote::OverwritePermissions;

    /// An update permissions operation.
    pub type UpdatePermissions = proto::quote::UpdatePermissions;

    /// An invoke compute operation.
    pub type InvokeCompute = proto::quote::InvokeCompute;

    /// A request to add funds to a user account's balance.
    pub type AddFundsRequest = proto::balance::AddFundsRequest;

    /// A response to a payments configuration request.
    pub type PaymentsConfigResponse = proto::config::PaymentsConfigResponse;

    impl TransparentProto for QuoteFees {}
    impl TransparentProto for SignedQuote {}
    impl TransparentProto for SignedReceipt {}
    impl TransparentProto for RetrieveValues {}
    impl TransparentProto for StoreValues {}
    impl TransparentProto for RetrievePermissions {}
    impl TransparentProto for OverwritePermissions {}
    impl TransparentProto for UpdatePermissions {}
    impl TransparentProto for InvokeCompute {}
    impl TransparentProto for AddFundsRequest {}
    impl TransparentProto for PaymentsConfigResponse {}

    /// A price quote.
    #[derive(Clone, Debug, PartialEq)]
    pub struct PriceQuote {
        /// A nonce that uniquely identifies this quote.
        pub nonce: Vec<u8>,

        /// The fees for this quote.
        pub fees: QuoteFees,

        /// The request that this quote is for.
        pub request: PriceQuoteRequest,

        /// The point in time at which this quote is no longer valid.
        pub expires_at: DateTime<Utc>,

        /// The preprocessing requirements for this operation.
        pub preprocessing_requirements: Vec<PreprocessingRequirement>,

        /// The auxiliary material requirements for this operation.
        pub auxiliary_material_requirements: Vec<AuxiliaryMaterialRequirement>,
    }

    impl ConvertProto for PriceQuote {
        type ProtoType = proto::quote::PriceQuote;

        fn into_proto(self) -> Self::ProtoType {
            Self::ProtoType {
                nonce: self.nonce,
                fees: Some(self.fees),
                request: Some(self.request.into_proto()),
                expires_at: Some(self.expires_at.into_proto()),
                preprocessing_requirements: self.preprocessing_requirements.into_proto(),
                auxiliary_material_requirements: self.auxiliary_material_requirements.into_proto(),
            }
        }

        fn try_from_proto(model: Self::ProtoType) -> Result<Self, ProtoError> {
            let nonce = model.nonce;
            let fees = model.fees.ok_or(ProtoError("'fees' not set"))?;
            let request = model.request.ok_or(ProtoError("'request' not set"))?.try_into_rust()?;
            let preprocessing_requirements = model.preprocessing_requirements.try_into_rust()?;
            let auxiliary_material_requirements = model.auxiliary_material_requirements.try_into_rust()?;
            let expires_at = model
                .expires_at
                .ok_or(ProtoError("'expires_at' not set"))?
                .try_into_rust()
                .map_err(|_| ProtoError("invalid 'expires_at' field"))?;
            Ok(Self { nonce, fees, request, expires_at, preprocessing_requirements, auxiliary_material_requirements })
        }
    }

    /// A price quote request.
    #[derive(Clone, Debug, PartialEq)]
    pub enum PriceQuoteRequest {
        /// A request to get the preprocessing pool status.
        PoolStatus,

        /// A request to retrieve the permissions for a set of values.
        RetrievePermissions(RetrievePermissions),

        /// A request to set the permissions for a set of values.
        OverwritePermissions(OverwritePermissions),

        /// A request to update the permissions for a set of values.
        UpdatePermissions(UpdatePermissions),

        /// A request to retrieve a set of values.
        RetrieveValues(RetrieveValues),

        /// A request to store a program.
        StoreProgram(StoreProgram),

        /// A request to store a set of values.
        StoreValues(StoreValues),

        /// A request to invoke a computation.
        InvokeCompute(InvokeCompute),
    }

    impl ConvertProto for PriceQuoteRequest {
        type ProtoType = proto::quote::PriceQuoteRequest;

        fn into_proto(self) -> Self::ProtoType {
            use proto::quote::price_quote_request::Operation as Proto;
            let operation = match self {
                Self::PoolStatus => Proto::PoolStatus(()),
                Self::RetrievePermissions(op) => Proto::RetrievePermissions(op.into_proto()),
                Self::OverwritePermissions(op) => Proto::OverwritePermissions(op.into_proto()),
                Self::UpdatePermissions(op) => Proto::UpdatePermissions(op.into_proto()),
                Self::RetrieveValues(op) => Proto::RetrieveValues(op.into_proto()),
                Self::StoreProgram(op) => Proto::StoreProgram(op.into_proto()),
                Self::StoreValues(op) => Proto::StoreValues(op.into_proto()),
                Self::InvokeCompute(op) => Proto::InvokeCompute(op.into_proto()),
            };
            Self::ProtoType { operation: Some(operation) }
        }

        fn try_from_proto(model: Self::ProtoType) -> Result<Self, ProtoError> {
            use proto::quote::price_quote_request::Operation as Proto;
            let operation = model.operation.ok_or(ProtoError("'operation' not set"))?;
            match operation {
                Proto::PoolStatus(()) => Ok(Self::PoolStatus),
                Proto::RetrievePermissions(op) => Ok(Self::RetrievePermissions(op.try_into_rust()?)),
                Proto::OverwritePermissions(op) => Ok(Self::OverwritePermissions(op.try_into_rust()?)),
                Proto::UpdatePermissions(op) => Ok(Self::UpdatePermissions(op.try_into_rust()?)),
                Proto::RetrieveValues(op) => Ok(Self::RetrieveValues(op.try_into_rust()?)),
                Proto::StoreProgram(op) => Ok(Self::StoreProgram(op.try_into_rust()?)),
                Proto::StoreValues(op) => Ok(Self::StoreValues(op.try_into_rust()?)),
                Proto::InvokeCompute(op) => Ok(Self::InvokeCompute(op.try_into_rust()?)),
            }
        }
    }

    /// A store program operation.
    #[derive(Clone, Debug, PartialEq)]
    pub struct StoreProgram {
        /// The program's metadata.
        pub metadata: ProgramMetadata,

        /// A sha256 hash of the compiled program.
        pub contents_sha256: Vec<u8>,

        /// The program's name.
        pub name: String,
    }

    impl ConvertProto for StoreProgram {
        type ProtoType = super::proto::quote::StoreProgram;

        fn into_proto(self) -> Self::ProtoType {
            Self::ProtoType {
                metadata: Some(self.metadata.into_proto()),
                contents_sha256: self.contents_sha256,
                name: self.name,
            }
        }

        fn try_from_proto(model: Self::ProtoType) -> Result<Self, ProtoError> {
            let metadata = model.metadata.ok_or(ProtoError("'metadata' not set"))?.try_into_rust()?;
            Ok(Self { metadata, contents_sha256: model.contents_sha256, name: model.name })
        }
    }

    /// The metadata about a program being stored.
    #[derive(Clone, Debug, PartialEq)]
    pub struct ProgramMetadata {
        /// The size of the program in bytes.
        pub program_size: u64,

        /// The amount of memory needed by the program.
        pub memory_size: u64,

        /// The total number of instructions in the program.
        pub instruction_count: u64,

        ///The number of instructions per type.
        pub instructions: HashMap<String, u64>,

        /// The preprocessing requirements.
        pub preprocessing_requirements: Vec<PreprocessingRequirement>,

        /// The auxiliary material requirements.
        pub auxiliary_material_requirements: Vec<AuxiliaryMaterialRequirement>,
    }

    impl ConvertProto for ProgramMetadata {
        type ProtoType = super::proto::quote::ProgramMetadata;

        fn into_proto(self) -> Self::ProtoType {
            Self::ProtoType {
                program_size: self.program_size,
                memory_size: self.memory_size,
                instruction_count: self.instruction_count,
                instructions: self.instructions,
                preprocessing_requirements: self.preprocessing_requirements.into_proto(),
                auxiliary_material_requirements: self.auxiliary_material_requirements.into_proto(),
            }
        }

        fn try_from_proto(model: Self::ProtoType) -> Result<Self, ProtoError> {
            Ok(Self {
                program_size: model.program_size,
                memory_size: model.memory_size,
                instruction_count: model.instruction_count,
                instructions: model.instructions,
                preprocessing_requirements: model.preprocessing_requirements.try_into_rust()?,
                auxiliary_material_requirements: model.auxiliary_material_requirements.try_into_rust()?,
            })
        }
    }

    /// The number of preprocessing elements required for a program.
    #[derive(Clone, Debug, PartialEq)]
    pub struct PreprocessingRequirement {
        /// The preprocessing element.
        pub element: PreprocessingElement,

        /// The total number of elements of this type needed.
        pub count: u64,
    }

    impl ConvertProto for PreprocessingRequirement {
        type ProtoType = super::proto::quote::PreprocessingRequirement;

        fn into_proto(self) -> Self::ProtoType {
            Self::ProtoType { element: self.element.into_proto() as i32, count: self.count }
        }

        fn try_from_proto(model: Self::ProtoType) -> Result<Self, ProtoError> {
            let element =
                PreprocessingElement::try_from(model.element).map_err(|_| ProtoError("invalid 'element' field"))?;
            Ok(Self { element, count: model.count })
        }
    }

    // The auxiliary material required for a program.
    #[derive(Clone, Debug, PartialEq)]
    pub struct AuxiliaryMaterialRequirement {
        /// The auxiliary material.
        pub material: AuxiliaryMaterial,

        /// The version needed.
        ///
        /// This field is used internally and should be ignored by the client.
        pub version: u32,
    }

    impl ConvertProto for AuxiliaryMaterialRequirement {
        type ProtoType = super::proto::quote::AuxiliaryMaterialRequirement;

        fn into_proto(self) -> Self::ProtoType {
            Self::ProtoType { material: self.material.into_proto() as i32, version: self.version }
        }

        fn try_from_proto(model: Self::ProtoType) -> Result<Self, ProtoError> {
            let material =
                AuxiliaryMaterial::try_from(model.material).map_err(|_| ProtoError("invalid 'material' field"))?;
            Ok(Self { material, version: model.version })
        }
    }

    /// A request to get a payment receipt.
    #[derive(Clone, Debug, PartialEq)]
    pub struct PaymentReceiptRequest {
        /// The signed quote.
        pub signed_quote: SignedQuote,

        /// The transaction hash where this operation was paid.
        pub tx_hash: Option<String>,
    }

    impl ConvertProto for PaymentReceiptRequest {
        type ProtoType = proto::receipt::PaymentReceiptRequest;

        fn into_proto(self) -> Self::ProtoType {
            Self::ProtoType { signed_quote: Some(self.signed_quote), tx_hash: self.tx_hash.unwrap_or_default() }
        }

        fn try_from_proto(model: Self::ProtoType) -> Result<Self, ProtoError> {
            let Self::ProtoType { tx_hash, signed_quote } = model;
            let signed_quote = signed_quote.ok_or(ProtoError("'signed_quote' not set"))?;
            let tx_hash = if tx_hash.is_empty() { None } else { Some(tx_hash) };
            Ok(Self { signed_quote, tx_hash })
        }
    }

    /// A receipt for a paid operation.
    #[derive(Clone, Debug, PartialEq)]
    pub struct Receipt {
        /// A unique identifier for this operation.
        pub identifier: Vec<u8>,

        /// The metadata for the operation in this receipt.
        pub metadata: OperationMetadata,

        /// The time at which this receipt expires.
        pub expires_at: DateTime<Utc>,
    }

    impl ConvertProto for Receipt {
        type ProtoType = proto::receipt::Receipt;

        fn into_proto(self) -> Self::ProtoType {
            Self::ProtoType {
                identifier: self.identifier,
                metadata: Some(self.metadata.into_proto()),
                expires_at: Some(self.expires_at.into_proto()),
            }
        }

        fn try_from_proto(model: Self::ProtoType) -> Result<Self, ProtoError> {
            let metadata = model.metadata.ok_or(ProtoError("'metadadata' not set"))?;
            let expires_at = model
                .expires_at
                .ok_or(ProtoError("'expires_at' not set"))?
                .try_into_rust()
                .map_err(|_| ProtoError("invalid 'expires_at' field"))?;
            Ok(Self { identifier: model.identifier, metadata: metadata.try_into_rust()?, expires_at })
        }
    }

    /// The metadata for the operation in a receipt.
    #[derive(Clone, Debug, PartialEq)]
    pub enum OperationMetadata {
        /// A preprocessing pool status operation.
        PoolStatus,

        /// A retrieve permissions operation.
        RetrievePermissions(RetrievePermissions),

        /// An overwrite permissions operation.
        OverwritePermissions(OverwritePermissions),

        /// An update permissions operation.
        UpdatePermissions(UpdatePermissions),

        /// A retrieve values operation.
        RetrieveValues(RetrieveValues),

        /// A store program operation.
        StoreProgram(StoreProgram),

        /// A store values operation.
        StoreValues(StoreValues),

        /// An invoke compute operation.
        InvokeCompute(InvokeComputeMetadata),
    }

    macro_rules! impl_operation_metadata_from {
        ($inner:ident) => {
            impl From<$inner> for OperationMetadata {
                fn from(value: $inner) -> Self {
                    Self::$inner(value)
                }
            }
        };
    }

    impl_operation_metadata_from!(RetrievePermissions);
    impl_operation_metadata_from!(OverwritePermissions);
    impl_operation_metadata_from!(UpdatePermissions);
    impl_operation_metadata_from!(RetrieveValues);
    impl_operation_metadata_from!(StoreProgram);
    impl_operation_metadata_from!(StoreValues);

    impl From<InvokeComputeMetadata> for OperationMetadata {
        fn from(value: InvokeComputeMetadata) -> Self {
            Self::InvokeCompute(value)
        }
    }

    impl ConvertProto for OperationMetadata {
        type ProtoType = proto::receipt::OperationMetadata;

        fn into_proto(self) -> Self::ProtoType {
            use proto::receipt::operation_metadata::Operation as Proto;
            let operation = match self {
                Self::PoolStatus => Proto::PoolStatus(()),
                Self::RetrievePermissions(op) => Proto::RetrievePermissions(op.into_proto()),
                Self::OverwritePermissions(op) => Proto::OverwritePermissions(op.into_proto()),
                Self::UpdatePermissions(op) => Proto::UpdatePermissions(op.into_proto()),
                Self::RetrieveValues(op) => Proto::RetrieveValues(op.into_proto()),
                Self::StoreProgram(op) => Proto::StoreProgram(op.into_proto()),
                Self::StoreValues(op) => Proto::StoreValues(op.into_proto()),
                Self::InvokeCompute(op) => Proto::InvokeCompute(op.into_proto()),
            };
            Self::ProtoType { operation: Some(operation) }
        }

        fn try_from_proto(model: Self::ProtoType) -> Result<Self, ProtoError> {
            use proto::receipt::operation_metadata::Operation as Proto;
            let operation = model.operation.ok_or(ProtoError("'operation' not set"))?;
            match operation {
                Proto::PoolStatus(()) => Ok(Self::PoolStatus),
                Proto::RetrievePermissions(op) => Ok(Self::RetrievePermissions(op.try_into_rust()?)),
                Proto::OverwritePermissions(op) => Ok(Self::OverwritePermissions(op.try_into_rust()?)),
                Proto::UpdatePermissions(op) => Ok(Self::UpdatePermissions(op.try_into_rust()?)),
                Proto::RetrieveValues(op) => Ok(Self::RetrieveValues(op.try_into_rust()?)),
                Proto::StoreProgram(op) => Ok(Self::StoreProgram(op.try_into_rust()?)),
                Proto::StoreValues(op) => Ok(Self::StoreValues(op.try_into_rust()?)),
                Proto::InvokeCompute(op) => Ok(Self::InvokeCompute(op.try_into_rust()?)),
            }
        }
    }

    /// An invoke compute operation metadata.
    #[derive(Clone, Debug, PartialEq)]
    pub struct InvokeComputeMetadata {
        /// The quote.
        pub quote: InvokeCompute,

        /// The selected preprocessing offsets for this operation.
        pub offsets: Vec<SelectedPreprocessingOffsets>,

        /// The selected auxiliary material.
        pub auxiliary_materials: Vec<SelectedAuxiliaryMaterial>,
    }

    impl ConvertProto for InvokeComputeMetadata {
        type ProtoType = proto::receipt::InvokeComputeMetadata;

        fn into_proto(self) -> Self::ProtoType {
            Self::ProtoType {
                quote: Some(self.quote),
                offsets: self.offsets.into_proto(),
                auxiliary_materials: self.auxiliary_materials.into_proto(),
            }
        }

        fn try_from_proto(model: Self::ProtoType) -> Result<Self, ProtoError> {
            let quote = model.quote.ok_or(ProtoError("'quote' not set"))?;
            let offsets = model.offsets.try_into_rust()?;
            let auxiliary_materials = model.auxiliary_materials.try_into_rust()?;
            Ok(Self { quote, offsets, auxiliary_materials })
        }
    }

    /// The selected offsets for a preprocessing element.
    #[derive(Clone, Debug, PartialEq)]
    pub struct SelectedPreprocessingOffsets {
        // The preprocessing element.
        pub element: PreprocessingElement,

        // The first offset in the range.
        pub start: u64,

        // The one-past-the-end offset in this range.
        pub end: u64,

        // The size of all batches involved in the selected range.
        pub batch_size: u64,
    }

    impl ConvertProto for SelectedPreprocessingOffsets {
        type ProtoType = proto::receipt::SelectedPreprocessingOffsets;

        fn into_proto(self) -> Self::ProtoType {
            Self::ProtoType {
                element: self.element.into(),
                start: self.start,
                end: self.end,
                batch_size: self.batch_size,
            }
        }

        fn try_from_proto(model: Self::ProtoType) -> Result<Self, ProtoError> {
            let element =
                PreprocessingElement::try_from(model.element).map_err(|_| ProtoError("invalid 'element' field"))?;
            Ok(Self { element, start: model.start, end: model.end, batch_size: model.batch_size })
        }
    }

    /// A request to add funds to a user's account.
    #[derive(Clone, Debug, PartialEq)]
    pub struct AddFundsPayload {
        /// The user the funds are being given to.
        pub recipient: UserId,

        /// A nonce that is used to add entropy to the hash of this message and to prevent duplicate spending.
        pub nonce: [u8; 32],
    }

    impl ConvertProto for AddFundsPayload {
        type ProtoType = proto::balance::AddFundsPayload;

        fn into_proto(self) -> Self::ProtoType {
            Self::ProtoType { recipient: Some(self.recipient.into_proto()), nonce: self.nonce.to_vec() }
        }

        fn try_from_proto(model: Self::ProtoType) -> Result<Self, ProtoError> {
            let recipient = model.recipient.ok_or(ProtoError("'recipient' not set"))?.try_into_rust()?;
            let nonce = model.nonce.try_into().map_err(|_| ProtoError("'nonce' must be 32 bytes long"))?;
            Ok(Self { recipient, nonce })
        }
    }

    /// The selected auxiliary material.
    #[derive(Clone, Debug, PartialEq)]
    pub struct SelectedAuxiliaryMaterial {
        /// The material type.
        pub material: AuxiliaryMaterial,

        /// The material version.
        pub version: u32,
    }

    impl ConvertProto for SelectedAuxiliaryMaterial {
        type ProtoType = proto::receipt::SelectedAuxiliaryMaterial;

        fn into_proto(self) -> Self::ProtoType {
            Self::ProtoType { material: self.material.into(), version: self.version }
        }

        fn try_from_proto(model: Self::ProtoType) -> Result<Self, ProtoError> {
            let material =
                AuxiliaryMaterial::try_from(model.material).map_err(|_| ProtoError("invalid 'material' field"))?;
            Ok(Self { material, version: model.version })
        }
    }

    /// The response to a request to get an account's balance.
    pub struct AccountBalanceResponse {
        /// The account balance, in unil.
        pub balance: u64,

        // The timestamp at which this balance was last updated.
        pub last_updated: DateTime<Utc>,

        // The timestamp at which this balance will expire.
        pub expires_at: DateTime<Utc>,
    }

    impl ConvertProto for AccountBalanceResponse {
        type ProtoType = proto::balance::AccountBalanceResponse;

        fn into_proto(self) -> Self::ProtoType {
            Self::ProtoType {
                balance: self.balance,
                last_updated: Some(self.last_updated.into_proto()),
                expires_at: Some(self.expires_at.into_proto()),
            }
        }

        fn try_from_proto(model: Self::ProtoType) -> Result<Self, ProtoError> {
            let last_updated = model.last_updated.ok_or(ProtoError("'last_updated' not set"))?.try_into_rust()?;
            let expires_at = model.expires_at.ok_or(ProtoError("'expire_at' not set"))?.try_into_rust()?;
            Ok(Self { balance: model.balance, last_updated, expires_at })
        }
    }
}
